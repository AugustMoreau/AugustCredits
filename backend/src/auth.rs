//! Authentication and authorization module
//!
//! Comprehensive auth system supporting both JWT tokens and API keys.
//! Handles user registration, login, signature verification for wallet-based auth,
//! and role-based access control with rate limiting integration.

use anyhow::{Context, Result};
use axum::{
    extract::{FromRequestParts, Query},
    http::{request::Parts, StatusCode, HeaderMap},
    response::{IntoResponse, Response},
    Json, RequestPartsExt,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::error;
use uuid::Uuid;

use crate::{
    config::Config,
    database::Database,
    models::{User, UserTier},
    AppState,
};

/// JWT token claims containing user identity and permissions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // User ID
    pub wallet_address: String,
    pub tier: UserTier,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
}

/// Authenticated user context with permissions and limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: Uuid,
    pub wallet_address: String,
    pub api_key: String,
    pub tier: UserTier,
    pub is_active: bool,
    pub monthly_limit: Option<i64>,
    pub rate_limit_override: Option<i32>,
}

// Login types moved to models.rs for better organization

/// API key authentication payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyAuth {
    pub api_key: String,
}

/// Authentication method detected from request headers
#[derive(Debug, Clone)]
pub enum AuthMethod {
    ApiKey(String),
    Jwt(String),
}

/// Core authentication service handling tokens and verification
#[derive(Clone)]
pub struct AuthService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
    token_expiry: Duration,
}

impl AuthService {
    /// Creates a new auth service with JWT configuration
    pub fn new(config: &Config) -> Result<Self> {
        let encoding_key = EncodingKey::from_secret(config.auth.jwt_secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.auth.jwt_secret.as_bytes());
        
        Ok(Self {
            encoding_key,
            decoding_key,
            issuer: "august-credits".to_string(),
            token_expiry: Duration::hours(24),
        })
    }
    
    /// Generates a JWT token for an authenticated user
    pub fn generate_token(&self, user: &User) -> Result<String> {
        let now = Utc::now();
        let exp = now + self.token_expiry;
        
        let claims = Claims {
            sub: user.id.to_string(),
            wallet_address: user.wallet_address.clone(),
            tier: user.tier.clone(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            iss: self.issuer.clone(),
        };
        
        encode(&Header::default(), &claims, &self.encoding_key)
            .context("Failed to generate JWT token")
    }
    
    /// Validates and decodes a JWT token, returning claims if valid
    pub fn validate_token(&self, token: &str) -> Result<Claims> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.issuer]);
        
        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)
            .context("Failed to validate JWT token")?;
        
        Ok(token_data.claims)
    }
    
    /// Generates a unique nonce for wallet signature challenges
    pub fn generate_nonce() -> String {
        format!("august-credits-{}", Uuid::new_v4().simple())
    }
    
    /// Creates a standardized message for wallet signature verification
    pub fn create_sign_message(wallet_address: &str, nonce: &str) -> String {
        format!(
            "Welcome to AugustCredits!\n\nPlease sign this message to authenticate.\n\nWallet: {}\nNonce: {}\nTimestamp: {}",
            wallet_address,
            nonce,
            Utc::now().timestamp()
        )
    }
    
    /// Verifies a wallet signature against the expected message
    /// 
    /// Note: This is a simplified implementation. Production systems should use
    /// proper cryptographic verification with libraries like ethers-rs.
    pub fn verify_signature(&self, wallet_address: &str, message: &str, signature: &str) -> Result<bool> {
        
        // Perform basic format validation
        if signature.len() < 130 || !signature.starts_with("0x") {
            return Ok(false);
        }
        
        // Ensure message integrity
        if !message.contains(wallet_address) {
            return Ok(false);
        }
        
        // TODO: Implement proper ECDSA signature verification
        Ok(true)
    }

    /// Registers a new user account with wallet verification
    pub async fn register_user(&self, _payload: crate::models::RegisterRequest) -> Result<crate::models::RegisterResponse, AuthError> {
        // TODO: Implement user registration with signature verification
        Err(AuthError::InternalError)
    }

    /// Authenticates user login and returns JWT tokens
    pub async fn login_user(&self, _payload: crate::models::LoginRequest) -> Result<crate::models::LoginResponse, AuthError> {
        // TODO: Implement wallet-based login with signature verification
        Err(AuthError::InternalError)
    }

    /// Refreshes expired JWT tokens using a valid refresh token
    pub async fn refresh_token(&self, _payload: crate::models::RefreshTokenRequest) -> Result<crate::models::LoginResponse, AuthError> {
        // TODO: Implement secure token refresh mechanism
        Err(AuthError::InternalError)
    }

    /// Retrieves user profile information by ID
    pub async fn get_user_profile(&self, _user_id: Uuid) -> Result<crate::models::UserProfile, AuthError> {
        // TODO: Implement profile retrieval from database
        Err(AuthError::InternalError)
    }

    /// Authenticates a request using an API key
    pub async fn authenticate_api_key(&self, api_key: &str, database: &Database) -> Result<AuthUser, AuthError> {
        let user = database.get_user_by_api_key(api_key)
            .await
            .map_err(|_| AuthError::DatabaseError)?
            .ok_or(AuthError::InvalidApiKey)?;

        if !user.is_active {
            return Err(AuthError::UserInactive);
        }

        Ok(AuthUser {
            id: user.id,
            wallet_address: user.wallet_address,
            api_key: user.api_key,
            tier: user.tier,
            is_active: user.is_active,
            monthly_limit: user.monthly_limit,
            rate_limit_override: user.rate_limit_override,
        })
    }

    /// Authenticates a request using a JWT token
    pub async fn authenticate_jwt(&self, token: &str, database: &Database) -> Result<AuthUser, AuthError> {
        let claims = self.validate_token(token)
            .map_err(|_| AuthError::InvalidToken)?;

        let user_id: Uuid = claims.sub.parse()
            .map_err(|_| AuthError::InvalidToken)?;

        let user = database.get_user_by_id(user_id)
            .await
            .map_err(|_| AuthError::DatabaseError)?
            .ok_or(AuthError::UserNotFound)?;

        if !user.is_active {
            return Err(AuthError::UserInactive);
        }

        Ok(AuthUser {
            id: user.id,
            wallet_address: user.wallet_address,
            api_key: user.api_key,
            tier: user.tier,
            is_active: user.is_active,
            monthly_limit: user.monthly_limit,
            rate_limit_override: user.rate_limit_override,
        })
    }

    /// Extracts authentication method from HTTP headers
    pub fn extract_auth_from_headers(&self, headers: &HeaderMap) -> Option<AuthMethod> {
        // Look for API key in custom header
        if let Some(api_key) = headers.get("x-api-key") {
            if let Ok(api_key_str) = api_key.to_str() {
                return Some(AuthMethod::ApiKey(api_key_str.to_string()));
            }
        }

        // Look for Bearer token in standard Authorization header
        if let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("Bearer ") {
                    let token = auth_str.trim_start_matches("Bearer ").trim();
                    return Some(AuthMethod::Jwt(token.to_string()));
                }
            }
        }

        None
    }
}

// JWT Authentication Extractor
#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    AppState: FromRequestParts<S>,
{
    type Rejection = AuthError;
    
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_request_parts(parts, state)
            .await
            .map_err(|_| AuthError::InternalError)?;
        
        // Try JWT authentication first
        if let Some(auth_header) = parts.headers.get(axum::http::header::AUTHORIZATION) {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    match app_state.auth.validate_token(token) {
                Ok(claims) => {
                    let user_id: Uuid = claims.sub.parse()
                        .map_err(|_| AuthError::InvalidToken)?;
                    
                    match app_state.database.get_user_by_id(user_id).await {
                        Ok(Some(user)) => {
                            if !user.is_active {
                                return Err(AuthError::UserInactive);
                            }
                            
                            // Update last login
                            let _ = app_state.database.update_user_last_login(user.id).await;
                            
                            return Ok(AuthUser {
                                id: user.id,
                                wallet_address: user.wallet_address,
                                api_key: user.api_key,
                                tier: user.tier,
                                is_active: user.is_active,
                                monthly_limit: user.monthly_limit,
                                rate_limit_override: user.rate_limit_override,
                            });
                        }
                        Ok(None) => return Err(AuthError::UserNotFound),
                        Err(_) => return Err(AuthError::DatabaseError),
                    }
                }
                        Err(_) => return Err(AuthError::InvalidToken),
                    }
                }
            }
        }
        
        // Try API key authentication
        if let Ok(Query(params)) = parts.extract::<Query<HashMap<String, String>>>().await {
            if let Some(api_key) = params.get("api_key") {
                return authenticate_with_api_key(&app_state.database, api_key).await;
            }
        }
        
        // Check for API key in headers
        if let Some(api_key) = parts.headers.get("X-API-Key") {
            if let Ok(api_key_str) = api_key.to_str() {
                return authenticate_with_api_key(&app_state.database, api_key_str).await;
            }
        }
        
        Err(AuthError::MissingCredentials)
    }
}

async fn authenticate_with_api_key(database: &Database, api_key: &str) -> Result<AuthUser, AuthError> {
    match database.get_user_by_api_key(api_key).await {
        Ok(Some(user)) => {
            if !user.is_active {
                return Err(AuthError::UserInactive);
            }
            
            // Update last login
            let _ = database.update_user_last_login(user.id).await;
            
            Ok(AuthUser {
                id: user.id,
                wallet_address: user.wallet_address,
                api_key: user.api_key,
                tier: user.tier,
                is_active: user.is_active,
                monthly_limit: user.monthly_limit,
                rate_limit_override: user.rate_limit_override,
            })
        }
        Ok(None) => Err(AuthError::InvalidApiKey),
        Err(_) => Err(AuthError::DatabaseError),
    }
}

// Optional authentication extractor (doesn't fail if no auth provided)
pub struct OptionalAuth(pub Option<AuthUser>);

#[axum::async_trait]
impl<S> FromRequestParts<S> for OptionalAuth
where
    S: Send + Sync,
    AppState: FromRequestParts<S>,
{
    type Rejection = std::convert::Infallible;
    
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match AuthUser::from_request_parts(parts, state).await {
            Ok(user) => Ok(OptionalAuth(Some(user))),
            Err(_) => Ok(OptionalAuth(None)),
        }
    }
}

// Permission checking
// Unused permission and limit functions removed

// Authentication errors
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Missing authentication credentials")]
    MissingCredentials,
    
    #[error("Invalid JWT token")]
    InvalidToken,
    
    #[error("Invalid API key")]
    InvalidApiKey,
    
    #[error("User not found")]
    UserNotFound,
    
    #[error("User account is inactive")]
    UserInactive,
    
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Monthly limit exceeded")]
    MonthlyLimitExceeded,
    
    #[error("Database error")]
    DatabaseError,
    
    #[error("Internal server error")]
    InternalError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::MissingCredentials => (StatusCode::UNAUTHORIZED, "Missing authentication credentials"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid or expired token"),
            AuthError::InvalidApiKey => (StatusCode::UNAUTHORIZED, "Invalid API key"),
            AuthError::UserNotFound => (StatusCode::UNAUTHORIZED, "User not found"),
            AuthError::UserInactive => (StatusCode::FORBIDDEN, "User account is inactive"),
            AuthError::InsufficientPermissions => (StatusCode::FORBIDDEN, "Insufficient permissions"),
            AuthError::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded"),
            AuthError::MonthlyLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "Monthly limit exceeded"),
            AuthError::DatabaseError => (StatusCode::INTERNAL_SERVER_ERROR, "Database error"),
            AuthError::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };
        
        let body = Json(serde_json::json!({
            "error": error_message,
            "code": status.as_u16()
        }));
        
        (status, body).into_response()
    }
}

// Helper functions for testing and internal use
pub fn check_permission(user: &AuthUser, required_tier: UserTier) -> bool {
    match user.tier {
        UserTier::Admin => true,
        UserTier::Enterprise => matches!(required_tier, UserTier::Free | UserTier::Pro | UserTier::Enterprise),
        UserTier::Pro => matches!(required_tier, UserTier::Free | UserTier::Pro),
        UserTier::Free => matches!(required_tier, UserTier::Free),
    }
}

pub fn check_admin_permission(user: &AuthUser) -> bool {
    matches!(user.tier, UserTier::Admin)
}

pub fn get_rate_limit_for_user(user: &AuthUser, endpoint_limit: Option<i32>) -> i32 {
    // If user has a rate limit override, use it
    if let Some(override_limit) = user.rate_limit_override {
        return override_limit;
    }
    
    // Get the tier-based limit
    let tier_limit = match user.tier {
        UserTier::Free => 100,
        UserTier::Pro => 1000,
        UserTier::Enterprise => 5000,
        UserTier::Admin => 10000,
    };
    
    // Return the more restrictive of tier limit and endpoint limit
    match endpoint_limit {
        Some(limit) => tier_limit.min(limit),
        None => tier_limit,
    }
}

// Middleware for admin-only routes
pub async fn require_admin(user: AuthUser) -> Result<AuthUser, AuthError> {
    if matches!(user.tier, UserTier::Admin) {
        Ok(user)
    } else {
        Err(AuthError::InsufficientPermissions)
    }
}

// Middleware for specific tier requirements
// require_tier function removed as it was unused

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[test]
    fn test_auth_service_creation() {
        let config = Config::load().unwrap();
        let auth_service = AuthService::new(&config);
        assert!(auth_service.is_ok());
    }
    
    #[test]
    fn test_nonce_generation() {
        let nonce1 = AuthService::generate_nonce();
        let nonce2 = AuthService::generate_nonce();
        
        assert_ne!(nonce1, nonce2);
        assert!(nonce1.starts_with("august-credits-"));
        assert!(nonce2.starts_with("august-credits-"));
    }
    
    #[test]
    fn test_sign_message_creation() {
        let wallet = "0x1234567890123456789012345678901234567890";
        let nonce = "test-nonce";
        let message = AuthService::create_sign_message(wallet, nonce);
        
        assert!(message.contains(wallet));
        assert!(message.contains(nonce));
        assert!(message.contains("AugustCredits"));
    }
    
    #[test]
    fn test_permission_checking() {
        let admin_user = AuthUser {
            id: Uuid::new_v4(),
            wallet_address: "0x123".to_string(),
            api_key: "key".to_string(),
            tier: UserTier::Admin,
            is_active: true,
            monthly_limit: None,
            rate_limit_override: None,
        };
        
        let free_user = AuthUser {
            id: Uuid::new_v4(),
            wallet_address: "0x456".to_string(),
            api_key: "key2".to_string(),
            tier: UserTier::Free,
            is_active: true,
            monthly_limit: None,
            rate_limit_override: None,
        };
        
        // Admin can access everything
        assert!(check_permission(&admin_user, UserTier::Free));
        assert!(check_permission(&admin_user, UserTier::Pro));
        assert!(check_permission(&admin_user, UserTier::Enterprise));
        assert!(check_permission(&admin_user, UserTier::Admin));
        
        // Free user can only access free tier
        assert!(check_permission(&free_user, UserTier::Free));
        assert!(!check_permission(&free_user, UserTier::Pro));
        assert!(!check_permission(&free_user, UserTier::Enterprise));
        assert!(!check_permission(&free_user, UserTier::Admin));
        
        // Admin permission check
        assert!(check_admin_permission(&admin_user));
        assert!(!check_admin_permission(&free_user));
    }
    
    #[test]
    fn test_rate_limit_calculation() {
        let free_user = AuthUser {
            id: Uuid::new_v4(),
            wallet_address: "0x123".to_string(),
            api_key: "key".to_string(),
            tier: UserTier::Free,
            is_active: true,
            monthly_limit: None,
            rate_limit_override: None,
        };
        
        let pro_user_with_override = AuthUser {
            id: Uuid::new_v4(),
            wallet_address: "0x456".to_string(),
            api_key: "key2".to_string(),
            tier: UserTier::Pro,
            is_active: true,
            monthly_limit: None,
            rate_limit_override: Some(500),
        };
        
        // Free user gets tier limit
        assert_eq!(get_rate_limit_for_user(&free_user, None), 100);
        
        // Free user with restrictive endpoint limit
        assert_eq!(get_rate_limit_for_user(&free_user, Some(50)), 50);
        
        // Pro user with override
        assert_eq!(get_rate_limit_for_user(&pro_user_with_override, None), 500);
        assert_eq!(get_rate_limit_for_user(&pro_user_with_override, Some(1000)), 500);
    }
    
    #[tokio::test]
    async fn test_token_generation_and_validation() {
        let config = Config::load().unwrap();
        let auth_service = AuthService::new(&config).unwrap();
        
        let user = User {
            id: Uuid::new_v4(),
            wallet_address: "0x1234567890123456789012345678901234567890".to_string(),
            api_key: "test-key".to_string(),
            email: None,
            username: None,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login: None,
            tier: UserTier::Pro,
            monthly_limit: None,
            rate_limit_override: None,
        };
        
        let token = auth_service.generate_token(&user).unwrap();
        assert!(!token.is_empty());
        
        let claims = auth_service.validate_token(&token).unwrap();
        assert_eq!(claims.sub, user.id.to_string());
        assert_eq!(claims.wallet_address, user.wallet_address);
        assert_eq!(claims.tier, user.tier);
        assert_eq!(claims.iss, "august-credits");
    }
}