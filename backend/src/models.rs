//! Database models and schema definitions
//!
//! Complete data model for the AugustCredits platform, including user management,
//! API endpoint monetization, usage tracking, billing automation, and payment processing.
//! All models are designed for PostgreSQL with proper serialization support.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use uuid::Uuid;

/// User account management and authentication

/// Core user entity with wallet-based authentication
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub wallet_address: String,
    pub api_key: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub tier: UserTier,
    pub monthly_limit: Option<i64>,
    pub rate_limit_override: Option<i32>,
}

/// User subscription tiers with different access levels and limits
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "user_tier", rename_all = "lowercase")]
pub enum UserTier {
    Free,
    Pro,
    Enterprise,
    Admin,
}

/// Request payload for creating new user accounts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub wallet_address: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub tier: Option<UserTier>,
}

/// Request payload for updating existing user profiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub username: Option<String>,
    pub is_active: Option<bool>,
    pub tier: Option<UserTier>,
    pub monthly_limit: Option<i64>,
    pub rate_limit_override: Option<i32>,
}

/// API endpoint registration and monetization

/// Monetizable API endpoint with pricing and access controls
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiEndpoint {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub upstream_url: String,
    pub price_per_request: String, // Stored as string to handle large numbers
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub rate_limit: Option<i32>,
    pub rate_limit_window: Option<i32>, // seconds
    pub requires_auth: bool,
    pub allowed_methods: Vec<String>,
    pub request_timeout: Option<i32>, // seconds
    pub retry_attempts: Option<i32>,
}

/// Request payload for registering new API endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEndpointRequest {
    pub name: String,
    pub description: Option<String>,
    pub upstream_url: String,
    pub price_per_request: String,
    pub rate_limit: Option<i32>,
    pub rate_limit_window: Option<i32>,
    pub requires_auth: Option<bool>,
    pub allowed_methods: Option<Vec<String>>,
    pub request_timeout: Option<i32>,
    pub retry_attempts: Option<i32>,
}

/// Request payload for updating endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEndpointRequest {
    pub description: Option<String>,
    pub upstream_url: Option<String>,
    pub price_per_request: Option<String>,
    pub is_active: Option<bool>,
    pub rate_limit: Option<i32>,
    pub rate_limit_window: Option<i32>,
    pub requires_auth: Option<bool>,
    pub allowed_methods: Option<Vec<String>>,
    pub request_timeout: Option<i32>,
    pub retry_attempts: Option<i32>,
}

// Usage Tracking

/// Individual usage record for billing and analytics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UsageRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub endpoint_id: Uuid,
    pub request_count: i64,
    pub total_cost: String, // Stored as string to handle large numbers
    pub timestamp: DateTime<Utc>,
    pub billing_period: String, // e.g., "2024-01"
    pub status: UsageStatus,
    pub transaction_hash: Option<String>,
    pub gas_used: Option<String>,
    pub block_number: Option<i64>,
}

/// Status of usage records in the billing pipeline
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "usage_status", rename_all = "lowercase")]
pub enum UsageStatus {
    Pending,
    Billed,
    Failed,
    Refunded,
}

/// Detailed request logging for debugging and analytics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RequestLog {
    pub id: Uuid,
    pub user_id: Uuid,
    pub endpoint_id: Uuid,
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub status_code: i32,
    pub response_time_ms: i32,
    pub request_size: Option<i64>,
    pub response_size: Option<i64>,
    pub ip_address_hash: String,
    pub user_agent_hash: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub cost: String,
    pub error_message: Option<String>,
}

/// Request payload for creating request log entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRequestLogRequest {
    pub user_id: Uuid,
    pub endpoint_id: Uuid,
    pub request_id: String,
    pub method: String,
    pub path: String,
    pub status_code: i32,
    pub response_time_ms: i32,
    pub request_size: Option<i64>,
    pub response_size: Option<i64>,
    pub ip_address_hash: String,
    pub user_agent_hash: Option<String>,
    pub cost: String,
    pub error_message: Option<String>,
}

// Billing and Payments

/// Aggregated billing record for payment processing
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BillingRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub billing_period: String,
    pub total_requests: i64,
    pub total_cost: String,
    pub status: BillingStatus,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub transaction_hash: Option<String>,
    pub gas_used: Option<String>,
    pub block_number: Option<i64>,
    pub retry_count: i32,
    pub error_message: Option<String>,
}

/// Status of billing records in the payment pipeline
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "billing_status", rename_all = "lowercase")]
pub enum BillingStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

// Analytics

/// Platform-wide analytics and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsData {
    pub period: String,
    pub total_requests: i64,
    pub total_revenue: String,
    pub new_users: i64,
    pub active_users: i64,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

/// Performance and usage statistics for individual endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointStats {
    pub endpoint_id: Uuid,
    pub total_requests: i64,
    pub requests_today: i64,
    pub avg_response_time: f64,
    pub error_rate: f64,
    pub revenue: String,
}

// PaymentTransaction struct removed as it was unused

/// Types of blockchain transactions in the system
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "transaction_type", rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Payment,
    Refund,
    Fee,
}

/// Status of blockchain transactions
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "transaction_status", rename_all = "lowercase")]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    Cancelled,
}

// Analytics and Reporting

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DailyStats {
    pub id: Uuid,
    pub date: chrono::NaiveDate,
    pub endpoint_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub total_requests: i64,
    pub total_cost: String,
    pub unique_users: i32,
    pub avg_response_time: f64,
    pub error_rate: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageAnalytics {
    pub period: String,
    pub total_requests: i64,
    pub total_cost: String,
    pub unique_users: i32,
    pub top_endpoints: Vec<EndpointUsage>,
    pub top_users: Vec<UserUsage>,
    pub error_rate: f64,
    pub avg_response_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointUsage {
    pub endpoint_name: String,
    pub request_count: i64,
    pub total_cost: String,
    pub unique_users: i32,
    pub avg_response_time: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUsage {
    pub user_id: Uuid,
    pub wallet_address: String,
    pub request_count: i64,
    pub total_cost: String,
    pub endpoints_used: i32,
    pub avg_response_time: f64,
}

// Rate Limiting

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RateLimitEntry {
    pub id: Uuid,
    pub user_id: Uuid,
    pub endpoint_id: Uuid,
    pub window_start: DateTime<Utc>,
    pub request_count: i32,
    pub limit_exceeded: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// API Keys and Authentication

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub key_hash: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub usage_count: i64,
    pub rate_limit_override: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub permissions: Option<Vec<String>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub rate_limit_override: Option<i32>,
}

// System Configuration

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SystemConfig {
    pub key: String,
    pub value: String,
    pub description: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Uuid,
}

// Webhook Configuration

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WebhookEndpoint {
    pub id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub secret: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_triggered: Option<DateTime<Utc>>,
    pub failure_count: i32,
    pub max_retries: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub status: WebhookStatus,
    pub response_code: Option<i32>,
    pub response_body: Option<String>,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub next_retry: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "webhook_status", rename_all = "lowercase")]
pub enum WebhookStatus {
    Pending,
    Delivered,
    Failed,
    Cancelled,
}

// Response DTOs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub wallet_address: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub is_active: bool,
    pub tier: UserTier,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub monthly_limit: Option<i64>,
    pub current_usage: i64,
    pub balance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub upstream_url: String,
    pub price_per_request: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub rate_limit: Option<i32>,
    pub rate_limit_window: Option<i32>,
    pub total_requests: i64,
    pub total_revenue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageResponse {
    pub period: String,
    pub endpoint_name: String,
    pub request_count: i64,
    pub total_cost: String,
    pub avg_response_time: f64,
    pub error_rate: f64,
    pub last_request: Option<DateTime<Utc>>,
}

// Pagination

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub sort_by: Option<String>,
    pub sort_order: Option<SortOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: i64,
    pub page: u32,
    pub limit: u32,
    pub total_pages: u32,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, total: i64, page: u32, limit: u32) -> Self {
        let total_pages = ((total as f64) / (limit as f64)).ceil() as u32;
        Self {
            data,
            total,
            page,
            limit,
            total_pages,
        }
    }
}

// Error types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub code: Option<String>,
    pub details: Option<serde_json::Value>,
}

// ErrorResponse methods removed as they were unused

// Default implementations

impl Default for UserTier {
    fn default() -> Self {
        UserTier::Free
    }
}

impl Default for UsageStatus {
    fn default() -> Self {
        UsageStatus::Pending
    }
}

impl Default for BillingStatus {
    fn default() -> Self {
        BillingStatus::Pending
    }
}

impl Default for TransactionStatus {
    fn default() -> Self {
        TransactionStatus::Pending
    }
}

impl Default for WebhookStatus {
    fn default() -> Self {
        WebhookStatus::Pending
    }
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::Desc
    }
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: Some(1),
            limit: Some(20),
            sort_by: None,
            sort_order: Some(SortOrder::Desc),
        }
    }
}

// Additional models for auth and metering services

/// Wallet-based login request with signature verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub wallet_address: String,
    pub signature: String,
    pub message: String,
    pub nonce: String,
}

/// Successful login response with JWT tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserResponse,
}

/// User registration request with wallet verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub wallet_address: String,
    pub signature: String,
    pub message: String,
    pub nonce: String,
    pub email: Option<String>,
    pub username: Option<String>,
}

/// Successful registration response with user data and tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub user: UserResponse,
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub wallet_address: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub tier: UserTier,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub monthly_limit: Option<i64>,
    pub current_usage: i64,
    pub balance: String,
}

/// User account balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBalance {
    pub user_id: Uuid,
    pub balance: String,
    pub pending_charges: String,
    pub last_updated: DateTime<Utc>,
}

/// Request to deposit funds via blockchain transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositRequest {
    pub amount: String,
    pub transaction_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositResponse {
    pub transaction_id: Uuid,
    pub amount: String,
    pub status: TransactionStatus,
    pub created_at: DateTime<Utc>,
}

/// Request to withdraw funds to external wallet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawRequest {
    pub amount: String,
    pub destination_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawResponse {
    pub transaction_id: Uuid,
    pub amount: String,
    pub destination_address: String,
    pub status: TransactionStatus,
    pub created_at: DateTime<Utc>,
}