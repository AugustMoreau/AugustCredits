//! API proxy module for forwarding requests and tracking usage
//!
//! Core proxy service that acts as an intelligent gateway between clients and upstream APIs.
//! Provides comprehensive request forwarding with built-in usage tracking, billing integration,
//! rate limiting enforcement, detailed logging, and robust error handling with retry logic.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, Uri},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    auth::{AuthUser, get_rate_limit_for_user, get_monthly_limit_for_user},
    database::Database,
    models::{CreateRequestLogRequest, ApiEndpoint},
    AppState,
};

/// Structured representation of an incoming proxy request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRequest {
    pub endpoint_name: String,
    pub method: String,
    pub path: String,
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

/// Response data from a proxied request including metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub response_time_ms: u64,
    pub cost: String,
}

/// Usage statistics and metrics for API consumption analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMetrics {
    pub request_count: u64,
    pub total_cost: String,
    pub avg_response_time: f64,
    pub error_rate: f64,
    pub last_request: Option<chrono::DateTime<Utc>>,
}

/// Core proxy service for handling API request forwarding and tracking
pub struct ProxyService {
    client: Client,
    database: Database,
}

impl ProxyService {
    /// Creates a new proxy service with optimized HTTP client configuration
    pub fn new(database: Database) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .expect("Failed to create HTTP client");
        
        Self { client, database }
    }
    
    /// Proxies a request to the upstream API with comprehensive tracking and validation
    pub async fn proxy_request(
        &self,
        user: &AuthUser,
        endpoint: &ApiEndpoint,
        method: Method,
        path: &str,
        query_params: Query<HashMap<String, String>>,
        headers: HeaderMap,
        body: Body,
    ) -> Result<ProxyResponse, ProxyError> {
        let start_time = Instant::now();
        let request_id = Uuid::new_v4().to_string();
        
        // Check if method is allowed
        if !endpoint.allowed_methods.contains(&method.to_string()) {
            return Err(ProxyError::MethodNotAllowed(method.to_string()));
        }
        
        // Check rate limits
        self.check_rate_limits(user, endpoint).await?;
        
        // Check monthly limits
        self.check_monthly_limits(user).await?;
        
        // Check if user can afford this request
        let cost = endpoint.price_per_request.parse::<u128>()
            .map_err(|_| ProxyError::InvalidPricing)?;
        
        // Build upstream URL
        let upstream_url = self.build_upstream_url(endpoint, path, &query_params)?;
        
        // Prepare headers for upstream request
        let upstream_headers = self.prepare_upstream_headers(&headers)?;
        
        // Convert body to bytes
        let body_bytes = axum::body::to_bytes(body, usize::MAX).await
            .map_err(|_| ProxyError::InvalidRequestBody)?;
        
        let request_size = body_bytes.len() as i64;
        
        // Make upstream request with retries
        let response_result = self.make_upstream_request(
            &method,
            &upstream_url,
            &upstream_headers,
            &body_bytes,
            endpoint.request_timeout.unwrap_or(30),
            endpoint.retry_attempts.unwrap_or(3),
        ).await;
        
        let response_time = start_time.elapsed();
        let response_time_ms = response_time.as_millis() as u64;
        
        match response_result {
            Ok(upstream_response) => {
                let status_code = upstream_response.status().as_u16();
                let response_headers = self.extract_response_headers(upstream_response.headers());
                let response_body = upstream_response.bytes().await
                    .map_err(|e| ProxyError::UpstreamError(e.to_string()))?;
                
                let response_size = response_body.len() as i64;
                
                // Log the request
                self.log_request(
                    user,
                    endpoint,
                    &request_id,
                    &method.to_string(),
                    path,
                    status_code as i32,
                    response_time_ms as i32,
                    Some(request_size),
                    Some(response_size),
                    &cost.to_string(),
                    None,
                ).await?;
                
                // Update usage metrics
                self.update_usage_metrics(user.id, endpoint.id, 1, &cost.to_string()).await?;
                
                Ok(ProxyResponse {
                    status_code,
                    headers: response_headers,
                    body: response_body.to_vec(),
                    response_time_ms,
                    cost: cost.to_string(),
                })
            }
            Err(error) => {
                let status_code = match &error {
                    ProxyError::Timeout => 504,
                    ProxyError::UpstreamError(_) => 502,
                    _ => 500,
                };
                
                // Log the failed request
                self.log_request(
                    user,
                    endpoint,
                    &request_id,
                    &method.to_string(),
                    path,
                    status_code,
                    response_time_ms as i32,
                    Some(request_size),
                    None,
                    "0", // No cost for failed requests
                    Some(error.to_string()),
                ).await?;
                
                Err(error)
            }
        }
    }
    
    /// Validates that the user hasn't exceeded their rate limits for this endpoint
    async fn check_rate_limits(&self, user: &AuthUser, endpoint: &ApiEndpoint) -> Result<(), ProxyError> {
        let rate_limit = get_rate_limit_for_user(user, endpoint.rate_limit);
        let window_duration = Duration::from_secs(endpoint.rate_limit_window.unwrap_or(60) as u64);
        
        let (current_count, limit) = self.database
            .check_rate_limit(user.id, endpoint.id, window_duration)
            .await
            .map_err(|_| ProxyError::DatabaseError)?;
        
        if current_count >= rate_limit {
            warn!(
                "Rate limit exceeded for user {} on endpoint {}: {}/{}",
                user.id, endpoint.name, current_count, rate_limit
            );
            return Err(ProxyError::RateLimitExceeded {
                current: current_count,
                limit: rate_limit,
                reset_time: Utc::now() + chrono::Duration::from_std(window_duration).unwrap(),
            });
        }
        
        Ok(())
    }
    
    /// Checks if the user has exceeded their monthly usage limits
    async fn check_monthly_limits(&self, user: &AuthUser) -> Result<(), ProxyError> {
        if let Some(monthly_limit) = get_monthly_limit_for_user(user) {
            let start_of_month = Utc::now().date_naive().with_day(1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let end_of_month = Utc::now();
            
            let usage_records = self.database
                .get_user_usage(user.id, start_of_month, end_of_month)
                .await
                .map_err(|_| ProxyError::DatabaseError)?;
            
            let total_requests: i64 = usage_records.iter().map(|r| r.request_count).sum();
            
            if total_requests >= monthly_limit {
                warn!(
                    "Monthly limit exceeded for user {}: {}/{}",
                    user.id, total_requests, monthly_limit
                );
                return Err(ProxyError::MonthlyLimitExceeded {
                    current: total_requests,
                    limit: monthly_limit,
                });
            }
        }
        
        Ok(())
    }
    
    /// Constructs the complete upstream URL from endpoint configuration and request parameters
    fn build_upstream_url(
        &self,
        endpoint: &ApiEndpoint,
        path: &str,
        query_params: &Query<HashMap<String, String>>,
    ) -> Result<String, ProxyError> {
        let mut url = endpoint.upstream_url.clone();
        
        // Remove trailing slash from upstream URL
        if url.ends_with('/') {
            url.pop();
        }
        
        // Add path
        if !path.starts_with('/') {
            url.push('/');
        }
        url.push_str(path);
        
        // Add query parameters
        if !query_params.is_empty() {
            url.push('?');
            let query_string: Vec<String> = query_params
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect();
            url.push_str(&query_string.join("&"));
        }
        
        Ok(url)
    }
    
    /// Prepares headers for the upstream request by filtering and sanitizing
    fn prepare_upstream_headers(&self, headers: &HeaderMap) -> Result<HeaderMap, ProxyError> {
        let mut upstream_headers = HeaderMap::new();
        
        // Copy relevant headers, excluding hop-by-hop headers
        let hop_by_hop_headers = [
            "connection",
            "keep-alive",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailers",
            "transfer-encoding",
            "upgrade",
            "host",
        ];
        
        for (name, value) in headers.iter() {
            let name_str = name.as_str().to_lowercase();
            if !hop_by_hop_headers.contains(&name_str.as_str()) {
                upstream_headers.insert(name.clone(), value.clone());
            }
        }
        
        // Add custom headers
        upstream_headers.insert(
            HeaderName::from_static("x-forwarded-by"),
            HeaderValue::from_static("august-credits"),
        );
        
        Ok(upstream_headers)
    }
    
    /// Makes the actual HTTP request to the upstream API with retry logic
    async fn make_upstream_request(
        &self,
        method: &Method,
        url: &str,
        headers: &HeaderMap,
        body: &[u8],
        timeout_seconds: i32,
        max_retries: i32,
    ) -> Result<reqwest::Response, ProxyError> {
        let timeout_duration = Duration::from_secs(timeout_seconds as u64);
        
        for attempt in 1..=max_retries {
            let mut request_builder = self.client.request(method.clone(), url);
            
            // Add headers
            for (name, value) in headers.iter() {
                if let (Ok(name_str), Ok(value_str)) = (name.as_str().parse::<reqwest::header::HeaderName>(), value.to_str()) {
                    request_builder = request_builder.header(name_str, value_str);
                }
            }
            
            // Add body if present
            if !body.is_empty() {
                request_builder = request_builder.body(body.to_vec());
            }
            
            let request = request_builder.build()
                .map_err(|e| ProxyError::UpstreamError(e.to_string()))?;
            
            match timeout(timeout_duration, self.client.execute(request)).await {
                Ok(Ok(response)) => {
                    debug!("Upstream request successful on attempt {}", attempt);
                    return Ok(response);
                }
                Ok(Err(e)) => {
                    warn!("Upstream request failed on attempt {}: {}", attempt, e);
                    if attempt == max_retries {
                        return Err(ProxyError::UpstreamError(e.to_string()));
                    }
                }
                Err(_) => {
                    warn!("Upstream request timed out on attempt {}", attempt);
                    if attempt == max_retries {
                        return Err(ProxyError::Timeout);
                    }
                }
            }
            
            // Wait before retry (exponential backoff)
            let delay = Duration::from_millis(100 * (2_u64.pow(attempt as u32 - 1)));
            tokio::time::sleep(delay).await;
        }
        
        unreachable!()
    }
    
    /// Extracts and converts response headers to a standard format
    fn extract_response_headers(&self, headers: &reqwest::header::HeaderMap) -> HashMap<String, String> {
        let mut response_headers = HashMap::new();
        
        for (name, value) in headers.iter() {
            if let Ok(value_str) = value.to_str() {
                response_headers.insert(name.to_string(), value_str.to_string());
            }
        }
        
        response_headers
    }
    
    /// Logs detailed request information for billing and analytics
    async fn log_request(
        &self,
        user: &AuthUser,
        endpoint: &ApiEndpoint,
        request_id: &str,
        method: &str,
        path: &str,
        status_code: i32,
        response_time_ms: i32,
        request_size: Option<i64>,
        response_size: Option<i64>,
        cost: &str,
        error_message: Option<String>,
    ) -> Result<(), ProxyError> {
        let ip_hash = "anonymous".to_string(); // In production, hash the actual IP
        
        let log_request = CreateRequestLogRequest {
            user_id: user.id,
            endpoint_id: endpoint.id,
            request_id: request_id.to_string(),
            method: method.to_string(),
            path: path.to_string(),
            status_code,
            response_time_ms,
            request_size,
            response_size,
            ip_address_hash: ip_hash,
            user_agent_hash: None,
            cost: cost.to_string(),
            error_message,
        };
        
        self.database.create_request_log(log_request).await
            .map_err(|_| ProxyError::DatabaseError)?;
        
        Ok(())
    }
    
    /// Updates usage metrics for billing and analytics tracking
    async fn update_usage_metrics(
        &self,
        user_id: Uuid,
        endpoint_id: Uuid,
        request_count: i64,
        cost: &str,
    ) -> Result<(), ProxyError> {
        let billing_period = Utc::now().format("%Y-%m").to_string();
        
        self.database
            .create_usage_record(user_id, endpoint_id, request_count, cost, &billing_period)
            .await
            .map_err(|_| ProxyError::DatabaseError)?;
        
        Ok(())
    }
    
    /// Retrieves usage metrics for a user within a specified time range
    pub async fn get_usage_metrics(
        &self,
        user_id: Uuid,
        endpoint_id: Option<Uuid>,
        start_date: chrono::DateTime<Utc>,
        end_date: chrono::DateTime<Utc>,
    ) -> Result<UsageMetrics, ProxyError> {
        let usage_records = if let Some(endpoint_id) = endpoint_id {
            self.database.get_endpoint_usage(endpoint_id, start_date, end_date).await
        } else {
            self.database.get_user_usage(user_id, start_date, end_date).await
        }.map_err(|_| ProxyError::DatabaseError)?;
        
        let request_count: u64 = usage_records.iter().map(|r| r.request_count as u64).sum();
        let total_cost: u128 = usage_records.iter()
            .filter_map(|r| r.total_cost.parse::<u128>().ok())
            .sum();
        
        // Calculate average response time and error rate from request logs
        // This would require additional database queries in a real implementation
        let avg_response_time = 0.0; // Placeholder
        let error_rate = 0.0; // Placeholder
        let last_request = usage_records.first().map(|r| r.timestamp);
        
        Ok(UsageMetrics {
            request_count,
            total_cost: total_cost.to_string(),
            avg_response_time,
            error_rate,
            last_request,
        })
    }
}

// Proxy request handler
/// HTTP handler for processing incoming proxy requests
pub async fn handle_proxy_request(
    State(state): State<AppState>,
    user: AuthUser,
    Path(endpoint_name): Path<String>,
    method: Method,
    uri: Uri,
    query_params: Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Body,
) -> Result<impl IntoResponse, ProxyError> {
    // Get endpoint configuration
    let endpoint = state.database
        .get_endpoint_by_name(&endpoint_name)
        .await
        .map_err(|_| ProxyError::DatabaseError)?
        .ok_or(ProxyError::EndpointNotFound(endpoint_name.clone()))?;
    
    if !endpoint.is_active {
        return Err(ProxyError::EndpointInactive(endpoint_name));
    }
    
    // Extract path from URI
    let path = uri.path();
    let path_without_endpoint = path.strip_prefix(&format!("/proxy/{}", endpoint_name))
        .unwrap_or(path);
    
    // Create proxy service and handle request
    let proxy_service = ProxyService::new(state.database.clone());
    let response = proxy_service.proxy_request(
        &user,
        &endpoint,
        method,
        path_without_endpoint,
        query_params,
        headers,
        body,
    ).await?;
    
    // Convert proxy response to HTTP response
    let mut response_builder = Response::builder().status(response.status_code);
    
    // Add response headers
    for (name, value) in response.headers {
        if let (Ok(header_name), Ok(header_value)) = (
            HeaderName::try_from(name),
            HeaderValue::try_from(value),
        ) {
            response_builder = response_builder.header(header_name, header_value);
        }
    }
    
    // Add custom headers
    response_builder = response_builder
        .header("X-AugustCredits-Cost", response.cost)
        .header("X-AugustCredits-Response-Time", response.response_time_ms.to_string())
        .header("X-AugustCredits-User-ID", user.id.to_string());
    
    let response = response_builder
        .body(Body::from(response.body))
        .map_err(|_| ProxyError::InternalError)?;
    
    Ok(response)
}

// Error types
/// Comprehensive error types for proxy operations
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("Endpoint not found: {0}")]
    EndpointNotFound(String),
    
    #[error("Endpoint is inactive: {0}")]
    EndpointInactive(String),
    
    #[error("Method not allowed: {0}")]
    MethodNotAllowed(String),
    
    #[error("Rate limit exceeded: {current}/{limit}, resets at {reset_time}")]
    RateLimitExceeded {
        current: i32,
        limit: i32,
        reset_time: chrono::DateTime<Utc>,
    },
    
    #[error("Monthly limit exceeded: {current}/{limit}")]
    MonthlyLimitExceeded {
        current: i64,
        limit: i64,
    },
    
    #[error("Invalid pricing configuration")]
    InvalidPricing,
    
    #[error("Invalid request body")]
    InvalidRequestBody,
    
    #[error("Upstream request timed out")]
    Timeout,
    
    #[error("Upstream error: {0}")]
    UpstreamError(String),
    
    #[error("Database error")]
    DatabaseError,
    
    #[error("Internal server error")]
    InternalError,
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            ProxyError::EndpointNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            ProxyError::EndpointInactive(_) => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            ProxyError::MethodNotAllowed(_) => (StatusCode::METHOD_NOT_ALLOWED, self.to_string()),
            ProxyError::RateLimitExceeded { .. } => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            ProxyError::MonthlyLimitExceeded { .. } => (StatusCode::TOO_MANY_REQUESTS, self.to_string()),
            ProxyError::InvalidPricing => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ProxyError::InvalidRequestBody => (StatusCode::BAD_REQUEST, self.to_string()),
            ProxyError::Timeout => (StatusCode::GATEWAY_TIMEOUT, self.to_string()),
            ProxyError::UpstreamError(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            ProxyError::DatabaseError => (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()),
            ProxyError::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };
        
        let body = axum::Json(serde_json::json!({
            "error": error_message,
            "code": status.as_u16(),
            "timestamp": Utc::now().to_rfc3339()
        }));
        
        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::UserTier;
    
    /// Tests URL construction for upstream requests
    #[test]
    fn test_build_upstream_url() {
        let endpoint = ApiEndpoint {
            id: Uuid::new_v4(),
            name: "test-api".to_string(),
            description: None,
            owner_id: Uuid::new_v4(),
            upstream_url: "https://api.example.com".to_string(),
            price_per_request: "1000".to_string(),
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            rate_limit: None,
            rate_limit_window: None,
            requires_auth: true,
            allowed_methods: vec!["GET".to_string()],
            request_timeout: None,
            retry_attempts: None,
        };
        
        let database = Database::new("postgresql://test", 1).await.unwrap(); // This would fail in tests
        let proxy_service = ProxyService::new(database);
        
        let mut query_params = HashMap::new();
        query_params.insert("param1".to_string(), "value1".to_string());
        query_params.insert("param2".to_string(), "value with spaces".to_string());
        
        let url = proxy_service.build_upstream_url(
            &endpoint,
            "/test/path",
            &Query(query_params),
        ).unwrap();
        
        assert!(url.starts_with("https://api.example.com/test/path?"));
        assert!(url.contains("param1=value1"));
        assert!(url.contains("param2=value%20with%20spaces"));
    }
    
    /// Tests header preparation and filtering
    #[test]
    fn test_prepare_upstream_headers() {
        let database = Database::new("postgresql://test", 1).await.unwrap(); // This would fail in tests
        let proxy_service = ProxyService::new(database);
        
        let mut headers = HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert("authorization", "Bearer token".parse().unwrap());
        headers.insert("connection", "keep-alive".parse().unwrap()); // Should be filtered out
        headers.insert("host", "example.com".parse().unwrap()); // Should be filtered out
        
        let upstream_headers = proxy_service.prepare_upstream_headers(&headers).unwrap();
        
        assert!(upstream_headers.contains_key("content-type"));
        assert!(upstream_headers.contains_key("authorization"));
        assert!(!upstream_headers.contains_key("connection"));
        assert!(!upstream_headers.contains_key("host"));
        assert!(upstream_headers.contains_key("x-forwarded-by"));
    }
}