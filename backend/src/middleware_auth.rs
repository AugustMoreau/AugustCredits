//! Authentication middleware for AugustCredits
//!
//! HTTP middleware that intercepts incoming requests to validate authentication
//! credentials (API keys, JWT tokens) and inject authenticated user context
//! into request handlers for secure API access.

use crate::{
    auth::{AuthMethod, AuthService},
    error::AppResult,
    models::User,
    auth_error,
};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::Validation;
use tracing::warn;
use uuid::Uuid;



/// Main authentication middleware that validates credentials and injects user context
pub async fn auth_middleware(
    State(state): State<crate::AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_service = &state.auth;
    let mut request = request;
    let headers = request.headers();
    
    // Extract authentication method
    let auth_method = match auth_service.extract_auth_from_headers(headers) {
        Some(method) => method,
        None => {
            warn!("No authentication provided");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Authenticate user
    let user = match auth_method {
        AuthMethod::ApiKey(api_key) => {
            match auth_service.authenticate_api_key(&api_key, &state.database).await {
                Ok(user) => user,
                Err(e) => {
                    warn!("API key authentication failed: {}", e);
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
        }
        AuthMethod::Jwt(token) => {
            match auth_service.authenticate_jwt(&token, &state.database).await {
                Ok(user) => user,
                Err(e) => {
                    warn!("JWT authentication failed: {}", e);
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
        }
    };

    // Add user to request extensions
    request.extensions_mut().insert(user);
    
    // Continue to next middleware/handler
    Ok(next.run(request).await)
}



// get_user_from_request function removed as it was unused

/// Extracts and validates user ID from JWT token in Authorization header
pub fn extract_user_id(headers: &HeaderMap) -> AppResult<Uuid> {
    let auth_header = headers
        .get("Authorization")
        .ok_or_else(|| auth_error!("Missing Authorization header"))?
        .to_str()
        .map_err(|_| auth_error!("Invalid Authorization header"))?;

    if !auth_header.starts_with("Bearer ") {
        return Err(auth_error!("Invalid token format"));
    }

    let token = &auth_header[7..]; // Skip "Bearer " prefix
    let claims = jsonwebtoken::decode::<crate::auth::Claims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(b"your-secret-key"), // This should come from config
        &Validation::default(),
    )
    .map_err(|e| auth_error!(format!("Invalid token: {}", e)))?;

    Uuid::parse_str(&claims.claims.sub)
        .map_err(|_| auth_error!("Invalid user ID in token"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests JWT token generation and validation flow
    #[tokio::test]
    async fn test_jwt_token_generation_and_validation() {
        let config = crate::config::Config::load().unwrap();
        let auth_service = AuthService::new(&config).unwrap();
        
        let user = User {
            id: Uuid::new_v4(),
            wallet_address: "0x1234567890abcdef".to_string(),
            api_key: "test_key".to_string(),
            email: Some("test@example.com".to_string()),
            username: Some("testuser".to_string()),
            is_active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_login: None,
            tier: crate::models::UserTier::Free,
            monthly_limit: None,
            rate_limit_override: None,
        };

        // Generate token
        let token = auth_service.generate_token(&user).unwrap();
        assert!(!token.is_empty());

        // Validate token
        let claims = auth_service.validate_token(&token).unwrap();
        assert_eq!(claims.sub, user.id.to_string());
        assert_eq!(claims.wallet_address, user.wallet_address);
    }

    /// Tests authentication method extraction from headers
    #[tokio::test]
    async fn test_auth_method_extraction() {
        let config = crate::config::Config::load().unwrap();
        let auth_service = AuthService::new(&config).unwrap();
        
        let mut headers = HeaderMap::new();
        
        // Test API key extraction
        headers.insert("x-api-key", "test_api_key".parse().unwrap());
        let auth_method = auth_service.extract_auth_from_headers(&headers);
        if let Some(AuthMethod::ApiKey(key)) = auth_method {
            assert_eq!(key, "test_api_key");
        } else {
            panic!("Expected API key auth method");
        }
        
        // Test JWT token extraction
        headers.clear();
        headers.insert("authorization", "Bearer test_token".parse().unwrap());
        let auth_method = auth_service.extract_auth_from_headers(&headers);
        if let Some(AuthMethod::Jwt(token)) = auth_method {
            assert_eq!(token, "test_token");
        } else {
            panic!("Expected JWT auth method");
        }
    }
}