//! API Gateway service for AugustCredits
//!
//! Core request processing engine that handles authentication, rate limiting,
//! usage metering, and proxying to monetized API endpoints with comprehensive
//! logging and analytics.

use crate::{
    auth::AuthService,
    database::Database,
    error::{AppError, AppResult},
    metering::MeteringService,
    models::*,
};
use axum::{
    body::{Body, HttpBody},
    http::{HeaderMap, HeaderValue, Method, Uri},
    response::Response,
};
use reqwest::Client;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Main gateway service that processes and routes API requests
#[derive(Clone)]
pub struct GatewayService {
    client: Client,
    database: Arc<Database>,
    auth: Arc<AuthService>,
    metering: Arc<MeteringService>,
}

impl GatewayService {
    /// Creates a new gateway service with HTTP client and service dependencies
    pub fn new(
        database: Arc<Database>,
        auth: Arc<AuthService>,
        metering: Arc<MeteringService>,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            database,
            auth,
            metering,
        }
    }

    /// Processes incoming API requests with full authentication and metering
    pub async fn process_request(
        &self,
        endpoint_name: &str,
        method: Method,
        uri: Uri,
        headers: HeaderMap,
        body: Body,
    ) -> AppResult<Response<Body>> {
        let start_time = Instant::now();
        let request_id = Uuid::new_v4().to_string();

        debug!(
            "Processing request: {} {} {} (ID: {})",
            method, endpoint_name, uri, request_id
        );

        // Extract API key from headers
        let api_key = self.extract_api_key(&headers)?;

        // Authenticate user
        let user = self.auth.authenticate_api_key(&api_key, &self.database).await
            .map_err(|_| AppError::Auth("Invalid API key".to_string()))?;

        // Get endpoint configuration
        let endpoint = self.database
            .get_endpoint_by_name(endpoint_name)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Endpoint '{}' not found", endpoint_name)))?;

        if !endpoint.is_active {
            return Err(AppError::Validation("Endpoint is not active".to_string()));
        }

        // Check if method is allowed
        if !endpoint.allowed_methods.contains(&method.to_string()) {
            return Err(AppError::Validation(format!(
                "Method {} not allowed for this endpoint",
                method
            )));
        }

        // Check rate limits
        self.metering.check_rate_limit(user.id, endpoint.id).await?;

        // Convert body to bytes for size calculation and forwarding
        let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to read request body: {}", e);
                return Err(AppError::Internal("Failed to read request body".to_string()));
            }
        };

        let request_size = body_bytes.len() as i64;

        // Forward request to upstream
        let method_clone = method.clone();
        let uri_clone = uri.clone();
        let headers_clone = headers.clone();
        let response = self.forward_request(
            &endpoint,
            method_clone,
            uri_clone,
            headers_clone,
            body_bytes.clone(),
        ).await?;

        let response_time = start_time.elapsed().as_millis() as i32;
        let status_code = response.status().as_u16() as i32;
        let response_size = response.body().size_hint().lower() as i64;

        // Calculate cost
        let cost = self.calculate_cost(&endpoint.price_per_request)?;

        // Log the request
        let log_request = CreateRequestLogRequest {
            user_id: user.id,
            endpoint_id: endpoint.id,
            request_id: request_id.clone(),
            method: method.to_string(),
            path: uri.path().to_string(),
            status_code,
            response_time_ms: response_time,
            request_size: Some(request_size),
            response_size: Some(response_size),
            ip_address_hash: self.hash_ip_address(&headers),
            user_agent_hash: self.hash_user_agent(&headers),
            cost: cost.clone(),
            error_message: if status_code >= 400 {
                Some(format!("HTTP {}", status_code))
            } else {
                None
            },
        };

        // Log request asynchronously
        let database = self.database.clone();
        let metering = self.metering.clone();
        let user_id = user.id;
        let endpoint_id = endpoint.id;
        tokio::spawn(async move {
            if let Err(e) = database.create_request_log(log_request).await {
                error!("Failed to log request: {}", e);
            }

            // Update metering
            if let Err(e) = metering.record_request(user_id, endpoint_id, status_code, response_time).await {
                error!("Failed to update metering: {}", e);
            }
        });

        info!(
            "Request processed: {} {} {} -> {} ({}ms, {} bytes)",
            method, endpoint_name, uri, status_code, response_time, response_size
        );

        Ok(response)
    }

    /// Extract API key from request headers
    /// Extracts API key from request headers (Authorization or X-API-Key)
    fn extract_api_key(&self, headers: &HeaderMap) -> AppResult<String> {
        // Try Authorization header first (Bearer token)
        if let Some(auth_header) = headers.get("authorization") {
            let auth_str = auth_header.to_str()
                .map_err(|_| AppError::Auth("Invalid authorization header".to_string()))?;
            
            if auth_str.starts_with("Bearer ") {
                return Ok(auth_str[7..].to_string());
            }
        }

        // Try X-API-Key header
        if let Some(api_key_header) = headers.get("x-api-key") {
            return Ok(api_key_header.to_str()
                .map_err(|_| AppError::Auth("Invalid API key header".to_string()))?
                .to_string());
        }

        Err(AppError::Auth("API key not provided".to_string()))
    }

    /// Forward request to upstream endpoint
    /// Forwards authenticated requests to the target API endpoint
    async fn forward_request(
        &self,
        endpoint: &ApiEndpoint,
        method: Method,
        uri: Uri,
        mut headers: HeaderMap,
        body: bytes::Bytes,
    ) -> AppResult<Response<Body>> {
        // Build upstream URL
        let upstream_url = format!(
            "{}{}",
            endpoint.upstream_url.trim_end_matches('/'),
            uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("")
        );

        debug!("Forwarding to upstream: {} {}", method, upstream_url);

        // Remove hop-by-hop headers
        headers.remove("host");
        headers.remove("connection");
        headers.remove("proxy-authorization");
        headers.remove("proxy-authenticate");
        headers.remove("te");
        headers.remove("trailers");
        headers.remove("transfer-encoding");
        headers.remove("upgrade");

        // Add custom headers
        headers.insert("x-forwarded-by", HeaderValue::from_static("august-credits"));

        // Build request - convert axum Method to reqwest Method
        let reqwest_method = reqwest::Method::from_bytes(method.as_str().as_bytes())
            .map_err(|_| AppError::Internal("Invalid HTTP method".to_string()))?;
        let mut request_builder = self.client.request(reqwest_method, &upstream_url);

        // Add headers
        for (name, value) in headers.iter() {
            if let Ok(value_str) = value.to_str() {
                request_builder = request_builder.header(name.as_str(), value_str);
            }
        }

        // Add body if present
        if !body.is_empty() {
            request_builder = request_builder.body(body);
        }

        // Set timeout
        if let Some(timeout) = endpoint.request_timeout {
            request_builder = request_builder.timeout(Duration::from_secs(timeout as u64));
        }

        // Execute request with retries
        let mut last_error = None;
        let max_retries = endpoint.retry_attempts.unwrap_or(0) + 1;

        for attempt in 1..=max_retries {
            match request_builder.try_clone().unwrap().send().await {
                Ok(response) => {
                    debug!("Upstream response: {} (attempt {})", response.status(), attempt);
                    
                    // Convert reqwest::Response to axum::Response
                    let mut builder = Response::builder().status(response.status().as_u16());
                    
                    // Copy headers - convert from reqwest to axum
                    for (name, value) in response.headers() {
                        if let Ok(value_str) = value.to_str() {
                            if let Ok(header_name) = axum::http::HeaderName::from_bytes(name.as_str().as_bytes()) {
                                if let Ok(header_value) = axum::http::HeaderValue::from_str(value_str) {
                                    builder = builder.header(header_name, header_value);
                                }
                            }
                        }
                    }
                    
                    // Get body
                    let body_bytes = response.bytes().await
                        .map_err(|e| AppError::ExternalService(format!("Failed to read upstream response: {}", e)))?;
                    
                    return Ok(builder.body(Body::from(body_bytes))
                        .map_err(|e| AppError::Internal(format!("Failed to build response: {}", e)))?);
                }
                Err(e) => {
                    warn!("Upstream request failed (attempt {}): {}", attempt, e);
                    last_error = Some(e);
                    
                    if attempt < max_retries {
                        // Exponential backoff
                        let delay = Duration::from_millis(100 * (2_u64.pow((attempt - 1) as u32)));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(AppError::ExternalService(format!(
            "Upstream request failed after {} attempts: {}",
            max_retries,
            last_error.unwrap()
        )))
    }

    /// Calculate request cost
    /// Calculates the cost for a single API request
    fn calculate_cost(&self, price_per_request: &str) -> AppResult<String> {
        // For now, just return the price as-is
        // In a real implementation, you might apply discounts, taxes, etc.
        Ok(price_per_request.to_string())
    }

    /// Hash IP address for privacy
    /// Creates a privacy-preserving hash of the client IP address
    fn hash_ip_address(&self, headers: &HeaderMap) -> String {
        let ip = headers
            .get("x-forwarded-for")
            .or_else(|| headers.get("x-real-ip"))
            .and_then(|h| h.to_str().ok())
            .unwrap_or("unknown");
        
        // Simple hash for demo - use proper hashing in production
        format!("{:x}", md5::compute(ip.as_bytes()))
    }

    /// Hash user agent for privacy
    /// Creates a hash of the user agent for analytics while preserving privacy
    fn hash_user_agent(&self, headers: &HeaderMap) -> Option<String> {
        headers
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(|ua| format!("{:x}", md5::compute(ua.as_bytes())))
    }

    /// Get gateway statistics
    /// Retrieves comprehensive gateway performance statistics
    pub async fn get_stats(&self) -> AppResult<GatewayStats> {
        // This would typically aggregate data from the database
        // For now, return placeholder data
        Ok(GatewayStats {
            total_requests: 0,
            requests_today: 0,
            active_endpoints: 0,
            avg_response_time: 0.0,
            error_rate: 0.0,
        })
    }

    /// Gets detailed information about a specific API endpoint
    pub async fn get_endpoint_details(&self, endpoint_id: &Uuid) -> AppResult<ApiEndpoint> {
        self.database
            .get_endpoint_by_id(*endpoint_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))
    }

    /// Updates pricing and configuration for an API endpoint
    pub async fn update_endpoint_pricing(
        &self,
        user_id: Uuid,
        endpoint_id: &Uuid,
        request: UpdateEndpointRequest,
    ) -> AppResult<ApiEndpoint> {
        // Check if user owns the endpoint
        let endpoint = self.get_endpoint_details(endpoint_id).await?;
        if endpoint.owner_id != user_id {
            return Err(AppError::Auth("Not authorized to update this endpoint".to_string()));
        }

        self.database.update_endpoint(*endpoint_id, request).await
            .map_err(|e| AppError::Database(e))
    }

    /// Retrieves usage and performance statistics for an endpoint
    pub async fn get_endpoint_stats(
        &self,
        endpoint_id: &Uuid,
        _params: PaginationParams,
    ) -> AppResult<EndpointStats> {
        // In a real implementation, this would query usage statistics
        Ok(EndpointStats {
            endpoint_id: *endpoint_id,
            total_requests: 0,
            requests_today: 0,
            avg_response_time: 0.0,
            error_rate: 0.0,
            revenue: "0".to_string(),
        })
    }

    /// Lists all available API endpoints
    pub async fn list_endpoints(&self) -> AppResult<Vec<ApiEndpoint>> {
        // Placeholder implementation
        Ok(vec![])
    }

    /// Registers a new API endpoint for monetization
    pub async fn register_endpoint(&self, _user_id: Uuid, _payload: CreateEndpointRequest) -> AppResult<ApiEndpoint> {
        // Placeholder implementation
        Err(AppError::Database(anyhow::anyhow!("Not implemented")))
    }
}

/// Gateway statistics
/// Gateway performance and usage statistics
#[derive(Debug, serde::Serialize)]
pub struct GatewayStats {
    pub total_requests: u64,
    pub requests_today: u64,
    pub active_endpoints: u32,
    pub avg_response_time: f64,
    pub error_rate: f64,
}