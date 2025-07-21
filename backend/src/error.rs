//! Error handling for the AugustCredits backend
//!
//! Centralized error management system providing consistent error types,
//! HTTP status code mapping, and automatic error logging for the entire platform.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::fmt;
use tracing::error;

/// Comprehensive error type covering all platform operations
#[derive(Debug)]
pub enum AppError {
    /// Database-related errors
    Database(anyhow::Error),
    /// Blockchain-related errors
    Blockchain(anyhow::Error),
    /// Authentication/authorization errors
    Auth(String),
    /// Validation errors
    Validation(String),
    /// Rate limiting errors
    RateLimit(String),
    /// Payment/billing errors
    Payment(String),
    /// External service errors
    ExternalService(String),
    /// Configuration errors
    Config(String),
    /// Not found errors
    NotFound(String),
    /// Internal server errors
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Database(err) => write!(f, "Database error: {}", err),
            AppError::Blockchain(err) => write!(f, "Blockchain error: {}", err),
            AppError::Auth(msg) => write!(f, "Authentication error: {}", msg),
            AppError::Validation(msg) => write!(f, "Validation error: {}", msg),
            AppError::RateLimit(msg) => write!(f, "Rate limit error: {}", msg),
            AppError::Payment(msg) => write!(f, "Payment error: {}", msg),
            AppError::ExternalService(msg) => write!(f, "External service error: {}", msg),
            AppError::Config(msg) => write!(f, "Configuration error: {}", msg),
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

/// Converts application errors to proper HTTP responses with status codes
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message, error_code) = match &self {
            AppError::Database(_) => {
                error!("Database error: {}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string(), "DATABASE_ERROR")
            }
            AppError::Blockchain(_) => {
                error!("Blockchain error: {}", self);
                (StatusCode::BAD_GATEWAY, "Blockchain service unavailable".to_string(), "BLOCKCHAIN_ERROR")
            }
            AppError::Auth(msg) => {
                (StatusCode::UNAUTHORIZED, msg.clone(), "AUTH_ERROR")
            }
            AppError::Validation(msg) => {
                (StatusCode::BAD_REQUEST, msg.clone(), "VALIDATION_ERROR")
            }
            AppError::RateLimit(msg) => {
                (StatusCode::TOO_MANY_REQUESTS, msg.clone(), "RATE_LIMIT_ERROR")
            }
            AppError::Payment(msg) => {
                (StatusCode::PAYMENT_REQUIRED, msg.clone(), "PAYMENT_ERROR")
            }
            AppError::ExternalService(msg) => {
                error!("External service error: {}", self);
                (StatusCode::BAD_GATEWAY, msg.clone(), "EXTERNAL_SERVICE_ERROR")
            }
            AppError::Config(msg) => {
                error!("Configuration error: {}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone(), "CONFIG_ERROR")
            }
            AppError::NotFound(msg) => {
                (StatusCode::NOT_FOUND, msg.clone(), "NOT_FOUND")
            }
            AppError::Internal(msg) => {
                error!("Internal error: {}", self);
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone(), "INTERNAL_ERROR")
            }
        };

        let body = Json(json!({
            "success": false,
            "error": {
                "code": error_code,
                "message": error_message
            },
            "timestamp": chrono::Utc::now()
        }));

        (status, body).into_response()
    }
}

/// Convenience type alias for Results with AppError
/// Convenient result type for all application operations
pub type AppResult<T> = Result<T, AppError>;

/// Convert anyhow::Error to AppError::Database
/// Converts generic anyhow errors to application errors
impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        AppError::Database(err)
    }
}

/// Convert sqlx::Error to AppError::Database
/// Converts database errors to application errors
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(anyhow::Error::from(err))
    }
}

/// Convert serde_json::Error to AppError::Validation
/// Converts JSON serialization errors to application errors
impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Validation(format!("JSON parsing error: {}", err))
    }
}

/// Convert reqwest::Error to AppError::ExternalService
/// Converts HTTP client errors to application errors
impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::ExternalService(format!("HTTP request error: {}", err))
    }
}

/// Convert AuthError to AppError
/// Converts authentication errors to application errors
impl From<crate::auth::AuthError> for AppError {
    fn from(err: crate::auth::AuthError) -> Self {
        AppError::Auth(err.to_string())
    }
}

/// Helper macros for creating specific error types
/// Convenient macro for creating authentication errors
#[macro_export]
macro_rules! auth_error {
    ($msg:expr) => {
        $crate::error::AppError::Auth($msg.to_string())
    };
}

/// Convenient macro for creating validation errors
#[macro_export]
macro_rules! validation_error {
    ($msg:expr) => {
        $crate::error::AppError::Validation($msg.to_string())
    };
}

/// Convenient macro for creating not found errors
#[macro_export]
macro_rules! not_found_error {
    ($msg:expr) => {
        $crate::error::AppError::NotFound($msg.to_string())
    };
}

/// Convenient macro for creating internal server errors
#[macro_export]
macro_rules! internal_error {
    ($msg:expr) => {
        $crate::error::AppError::Internal($msg.to_string())
    };
}

/// Convenient macro for creating rate limit errors
#[macro_export]
macro_rules! rate_limit_error {
    ($msg:expr) => {
        $crate::error::AppError::RateLimit($msg.to_string())
    };
}

/// Convenient macro for creating payment errors
#[macro_export]
macro_rules! payment_error {
    ($msg:expr) => {
        $crate::error::AppError::Payment($msg.to_string())
    };
}