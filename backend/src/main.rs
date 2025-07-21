//! AugustCredits API Gateway
//!
//! High-performance API gateway and monetization platform that enables developers
//! to monetize their APIs through blockchain-based payments. Features comprehensive
//! user authentication, endpoint management, real-time usage tracking, rate limiting,
//! and seamless integration with Augustium smart contracts.

use anyhow::Result;
use axum::{
    extract::{Path, Query, Request, State},
    http::HeaderMap,
    middleware,
    response::Json,
    routing::{get, post, put}, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};
use tracing::info;

mod config;
mod database;
mod blockchain;
mod gateway;
mod metering;
mod auth;
mod middleware_auth;
mod metrics;
mod error;
mod models;

// Re-export commonly used types
pub use models::{
    LoginRequest, RegisterRequest, RefreshTokenRequest, UserProfile, 
    DepositRequest, WithdrawRequest
};

use config::Config;
use database::Database;
use blockchain::BlockchainClient;
use gateway::GatewayService;
use metering::MeteringService;
use auth::{AuthService, require_admin};
use metrics::MetricsService;
use error::{AppError, AppResult};

/// Shared application state containing all service instances
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub database: Arc<Database>,
    pub blockchain: Arc<BlockchainClient>,
    pub gateway: Arc<GatewayService>,
    pub metering: Arc<MeteringService>,
    pub auth: Arc<AuthService>,
    pub metrics: Arc<MetricsService>,
}

/// Standard API response wrapper for consistent JSON responses
#[derive(Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    /// Creates a successful API response with data
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }
    
    /// Creates an error API response with message
    pub fn error(error: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(error),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Query parameters for proxy requests
#[derive(Deserialize)]
struct ProxyQuery {
    endpoint: String,
}

/// Health check response with system status information
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    services: ServiceStatus,
}

/// Status of individual services for health monitoring
#[derive(Serialize)]
struct ServiceStatus {
    database: bool,
    blockchain: bool,
    redis: bool,
}

/// Main entry point for the AugustCredits API Gateway
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting AugustCredits API Gateway");

    // Load configuration
    let config = Arc::new(Config::load()?);
    info!("Configuration loaded successfully");

    // Initialize services
    let database = Arc::new(Database::new(&config.database_url, 10).await?);
    info!("Database connection established");

    let blockchain = Arc::new(BlockchainClient::new(&config).await?);
    info!("Blockchain client initialized");

    let auth: Arc<AuthService> = Arc::new(AuthService::new(&config)?);
    let metering: Arc<MeteringService> = Arc::new(MeteringService::new(database.clone()));
    let gateway = Arc::new(GatewayService::new(
        database.clone(),
        auth.clone(),
        metering.clone(),
    ));
    let metrics = Arc::new(MetricsService::new(database.clone()));

    info!("All services initialized successfully");

    // Create application state
    let state = AppState {
        config: config.clone(),
        database,
        blockchain,
        gateway,
        metering,
        auth,
        metrics,
    };

    // Build router
    let app = Router::new()
        // Health and status endpoints
        .route("/health", get(health_check))
        .route("/metrics", get(get_metrics))
        .route("/stats", get(get_usage_stats))
        
        // Authentication endpoints
        .route("/auth/register", post(register_user))
        .route("/auth/login", post(login_user))
        .route("/auth/refresh", post(refresh_token))
        
        // User management
        .route("/user/profile", get(get_user_profile))
        .route("/user/balance", get(get_user_balance))
        .route("/user/deposit", post(deposit_balance))
        .route("/user/withdraw", post(withdraw_balance))
        .route("/user/usage", get(get_user_usage))
        
        // API endpoint management
        .route("/endpoints", get(list_endpoints))
        .route("/endpoints", post(register_endpoint))
        .route("/endpoints/:id", get(get_endpoint_details))
        .route("/endpoints/:id/pricing", put(update_endpoint_pricing))
        .route("/endpoints/:id/stats", get(get_endpoint_stats))
        
        // Main proxy endpoint
        .route("/proxy/*path", get(proxy_request))
        .route("/proxy/*path", post(proxy_request))
        .route("/proxy/*path", axum::routing::put(proxy_request))
        .route("/proxy/*path", axum::routing::delete(proxy_request))
        
        // Admin endpoints
        .route("/admin/users", get(list_users))
        .route("/admin/billing", post(process_billing))
        .route("/admin/analytics", get(get_analytics))
        
        // Add middleware
        .layer(middleware::from_fn_with_state(
            state.clone(),
            middleware_auth::auth_middleware,
        ))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Start server
    let listener = TcpListener::bind(&config.server_address).await?;
    info!("Server listening on {}", config.server_address);
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

/// Returns the current health status of all system components
async fn health_check(State(state): State<AppState>) -> AppResult<Json<ApiResponse<HealthResponse>>> {
    let db_status = state.database.health_check().await.is_ok();
    let blockchain_status = state.blockchain.health_check().await.is_ok();
    let redis_status = true; // TODO: Implement Redis health check
    
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now(),
        services: ServiceStatus {
            database: db_status,
            blockchain: blockchain_status,
            redis: redis_status,
        },
    };
    
    Ok(Json(ApiResponse::success(response)))
}

/// Exposes system metrics in JSON format for monitoring
async fn get_metrics(State(state): State<AppState>) -> AppResult<String> {
    let metrics = state.metrics.get_metrics_snapshot().await;
    Ok(serde_json::to_string(&metrics).unwrap_or_else(|_| "{}".to_string()))
}

/// Retrieves current month's usage statistics for the authenticated user
async fn get_usage_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<ApiResponse<crate::metering::UserUsageStats>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let usage = state.metering.get_user_usage(user_id, crate::metering::UsagePeriod::Month).await?;
    Ok(Json(ApiResponse::success(usage)))
}

/// Creates a new user account with wallet integration
async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<crate::models::RegisterRequest>,
) -> AppResult<Json<ApiResponse<models::RegisterResponse>>> {
    let response = state.auth.register_user(payload).await?;
    Ok(Json(ApiResponse::success(response)))
}

/// Authenticates user credentials and returns access tokens
async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<ApiResponse<models::LoginResponse>>> {
    let response = state.auth.login_user(payload).await?;
    Ok(Json(ApiResponse::success(response)))
}

/// Generates new access tokens using a valid refresh token
async fn refresh_token(
    State(state): State<AppState>,
    Json(payload): Json<crate::models::RefreshTokenRequest>,
) -> AppResult<Json<ApiResponse<crate::models::LoginResponse>>> {
    let response = state.auth.refresh_token(payload).await?;
    Ok(Json(ApiResponse::success(response)))
}

/// Fetches detailed profile information for the authenticated user
async fn get_user_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<ApiResponse<UserProfile>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let profile = state.auth.get_user_profile(user_id).await?;
    Ok(Json(ApiResponse::success(profile)))
}

/// Returns the current account balance for the authenticated user
async fn get_user_balance(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<ApiResponse<crate::models::UserBalance>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let balance = state.metering.get_user_balance(user_id).await?;
    Ok(Json(ApiResponse::success(balance)))
}

/// Processes a balance deposit via blockchain transaction
async fn deposit_balance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DepositRequest>,
) -> AppResult<Json<ApiResponse<crate::models::DepositResponse>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let response = state.metering.deposit_balance(user_id, payload).await?;
    Ok(Json(ApiResponse::success(response)))
}

/// Initiates a balance withdrawal to user's wallet
async fn withdraw_balance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<WithdrawRequest>,
) -> AppResult<Json<ApiResponse<crate::models::WithdrawResponse>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let response = state.metering.withdraw_balance(user_id, payload).await?;
    Ok(Json(ApiResponse::success(response)))
}

/// Provides detailed usage analytics for the authenticated user
async fn get_user_usage(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<ApiResponse<crate::metering::UserUsageStats>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let usage = state.metering.get_user_usage(user_id, crate::metering::UsagePeriod::Month).await?;
    Ok(Json(ApiResponse::success(usage)))
}

/// Returns all publicly available API endpoints with their pricing
async fn list_endpoints(
    State(state): State<AppState>,
) -> AppResult<Json<ApiResponse<Vec<crate::models::ApiEndpoint>>>> {
    let endpoints = state.gateway.list_endpoints().await?;
    Ok(Json(ApiResponse::success(endpoints)))
}

/// Allows users to register their own API endpoints for monetization
async fn register_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<crate::models::CreateEndpointRequest>,
) -> AppResult<Json<ApiResponse<crate::models::ApiEndpoint>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let endpoint = state.gateway.register_endpoint(user_id, payload).await?;
    Ok(Json(ApiResponse::success(endpoint)))
}

/// Retrieves comprehensive information about a specific endpoint
async fn get_endpoint_details(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ApiResponse<models::ApiEndpoint>>> {
    let endpoint_id = uuid::Uuid::parse_str(&id)
        .map_err(|_| AppError::Validation("Invalid endpoint ID format".to_string()))?;
    let details = state.gateway.get_endpoint_details(&endpoint_id).await?;
    Ok(Json(ApiResponse::success(details)))
}

/// Updates pricing and configuration for user-owned endpoints
async fn update_endpoint_pricing(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<models::UpdateEndpointRequest>,
) -> AppResult<Json<ApiResponse<models::ApiEndpoint>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let endpoint_id = uuid::Uuid::parse_str(&id)
        .map_err(|_| AppError::Validation("Invalid endpoint ID format".to_string()))?;
    let endpoint = state.gateway.update_endpoint_pricing(user_id, &endpoint_id, payload).await?;
    Ok(Json(ApiResponse::success(endpoint)))
}

/// Provides usage analytics and performance metrics for an endpoint
async fn get_endpoint_stats(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<models::PaginationParams>,
) -> AppResult<Json<ApiResponse<models::EndpointStats>>> {
    let endpoint_id = uuid::Uuid::parse_str(&id)
        .map_err(|_| AppError::Validation("Invalid endpoint ID format".to_string()))?;
    let stats = state.gateway.get_endpoint_stats(&endpoint_id, params).await?;
    Ok(Json(ApiResponse::success(stats)))
}

/// Core proxy handler that routes requests to target APIs with metering
async fn proxy_request(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
) -> AppResult<axum::response::Response> {
    let (parts, body) = req.into_parts();
    let endpoint_name = match parts.uri.query() {
        Some(query) => {
            let params: ProxyQuery = serde_urlencoded::from_str(query).unwrap();
            params.endpoint
        }
        None => return Err(AppError::Validation("Missing endpoint query parameter".to_string()))
    };

    let response = state.gateway.process_request(
        &endpoint_name,
        parts.method,
        parts.uri,
        parts.headers,
        axum::body::Body::from(body),
    ).await?;
    
    Ok(response)
}

/// Admin endpoint to retrieve paginated list of all users
async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(pagination): Query<models::Pagination>,
) -> AppResult<Json<ApiResponse<Vec<models::User>>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let user = state.database.get_user_by_id(user_id).await?
        .ok_or_else(|| AppError::Auth("User not found".to_string()))?;
    let auth_user = crate::auth::AuthUser {
        id: user.id,
        wallet_address: user.wallet_address.clone(),
        api_key: "".to_string(),
        tier: user.tier,
        is_active: user.is_active,
        monthly_limit: user.monthly_limit,
        rate_limit_override: user.rate_limit_override,
    };
    require_admin(auth_user).await?;
    let users = state.database.list_users(pagination).await?;
    Ok(Json(ApiResponse::success(users)))
}

/// Admin endpoint to manually trigger billing cycle processing
async fn process_billing(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<ApiResponse<()>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let user = state.database.get_user_by_id(user_id).await?
        .ok_or_else(|| AppError::Auth("User not found".to_string()))?;
    let auth_user = crate::auth::AuthUser {
        id: user.id,
        wallet_address: user.wallet_address.clone(),
        api_key: "".to_string(),
        tier: user.tier,
        is_active: user.is_active,
        monthly_limit: user.monthly_limit,
        rate_limit_override: user.rate_limit_override,
    };
    require_admin(auth_user).await?;
    state.metering.process_billing(state.database.clone()).await?;
    Ok(Json(ApiResponse::success(())))
}

/// Admin endpoint providing platform-wide analytics and insights
async fn get_analytics(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(period): Query<crate::metering::UsagePeriod>,
) -> AppResult<Json<ApiResponse<models::AnalyticsData>>> {
    let user_id = middleware_auth::extract_user_id(&headers)?;
    let user = state.database.get_user_by_id(user_id).await?
        .ok_or_else(|| AppError::Auth("User not found".to_string()))?;
    let auth_user = crate::auth::AuthUser {
        id: user.id,
        wallet_address: user.wallet_address.clone(),
        api_key: "".to_string(),
        tier: user.tier,
        is_active: user.is_active,
        monthly_limit: user.monthly_limit,
        rate_limit_override: user.rate_limit_override,
    };
    require_admin(auth_user).await?;
    let analytics = state.metering.get_analytics(state.database.clone(), period).await?;
    Ok(Json(ApiResponse::success(analytics)))
}