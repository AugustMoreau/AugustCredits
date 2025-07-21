//! Database operations and connection management
//!
//! Provides a comprehensive database layer for the AugustCredits platform,
//! handling PostgreSQL connections, migrations, and all CRUD operations
//! for users, API endpoints, usage tracking, and billing records.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};

use sqlx::{
    postgres::{PgPool, PgPoolOptions},
    Row, Transaction, Postgres,
};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

use crate::models::*;

/// Main database service with connection pooling
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Creates a new database connection with optimized pool settings
    pub async fn new(database_url: &str, max_connections: u32) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(30))
            .connect(database_url)
            .await
            .context("Failed to connect to database")?;

        info!("Connected to database with {} max connections", max_connections);
        
        Ok(Self { pool })
    }
    
    /// Runs pending database migrations
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("Failed to run database migrations")?;
        
        info!("Database migrations completed successfully");
        Ok(())
    }
    
    /// Verifies database connectivity
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .context("Database health check failed")?;
        Ok(())
    }
    
    /// Returns the underlying connection pool for advanced operations
    pub fn get_pool(&self) -> &PgPool {
        &self.pool
    }

    /// Creates a test database instance with minimal connections
    #[cfg(test)]
    pub async fn new_test() -> Result<Self> {
        use crate::config::Config;
        let config = Config::load().context("Failed to load test config")?;
        
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&config.database_url)
            .await
            .context("Failed to connect to test database")?;
            
        Ok(Self { pool })
    }
    
    /// User account management operations
    
    /// Creates a new user account with auto-generated API key
    pub async fn create_user(&self, request: CreateUserRequest) -> Result<User> {
        let api_key = format!("ak_{}", Uuid::new_v4().simple());
        let now = Utc::now();
        
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (wallet_address, api_key, email, username, tier, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, wallet_address, api_key, email, username, is_active, created_at, updated_at, 
                      last_login, tier, monthly_limit, rate_limit_override
            "#
        )
        .bind(&request.wallet_address)
        .bind(&api_key)
        .bind(&request.email)
        .bind(&request.username)
        .bind(&request.tier.unwrap_or_default())
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create user")?;
        
        info!("Created user with ID: {}", user.id);
        Ok(user)
    }
    
    /// Retrieves user by their unique ID
    pub async fn get_user_by_id(&self, user_id: Uuid) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, wallet_address, api_key, email, username, is_active, created_at, updated_at,
                   last_login, tier, monthly_limit, rate_limit_override
            FROM users WHERE id = $1
            "#
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get user by ID")?;
        
        Ok(user)
    }
    
    /// Finds user by their API key for authentication
    pub async fn get_user_by_api_key(&self, api_key: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, wallet_address, api_key, email, username, is_active, created_at, updated_at,
                   last_login, tier, monthly_limit, rate_limit_override
            FROM users WHERE api_key = $1 AND is_active = true
            "#
        )
        .bind(api_key)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get user by API key")?;
        
        Ok(user)
    }
    
    /// Finds user by their wallet address for Web3 authentication
    pub async fn get_user_by_wallet(&self, wallet_address: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, wallet_address, api_key, email, username, is_active, created_at, updated_at,
                   last_login, tier, monthly_limit, rate_limit_override
            FROM users WHERE wallet_address = $1
            "#
        )
        .bind(wallet_address)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get user by wallet address")?;
        
        Ok(user)
    }
    
    /// Updates user profile information
    pub async fn update_user(&self, user_id: Uuid, request: UpdateUserRequest) -> Result<User> {
        let now = Utc::now();
        
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users SET
                email = COALESCE($2, email),
                username = COALESCE($3, username),
                is_active = COALESCE($4, is_active),
                tier = COALESCE($5, tier),
                monthly_limit = COALESCE($6, monthly_limit),
                rate_limit_override = COALESCE($7, rate_limit_override),
                updated_at = $8
            WHERE id = $1
            RETURNING id, wallet_address, api_key, email, username, is_active, created_at, updated_at,
                      last_login, tier, monthly_limit, rate_limit_override
            "#
        )
        .bind(user_id)
        .bind(request.email)
        .bind(request.username)
        .bind(request.is_active)
        .bind(request.tier)
        .bind(request.monthly_limit)
        .bind(request.rate_limit_override)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update user")?;
        
        Ok(user)
    }
    
    /// Lists all users with pagination support
    pub async fn list_users(&self, pagination: crate::models::Pagination) -> Result<Vec<User>> {
        let users = sqlx::query_as::<_, User>(
            r#"
            SELECT id, wallet_address, api_key, email, username, is_active, created_at, updated_at,
                   last_login, tier, monthly_limit, rate_limit_override
            FROM users
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#
        )
        .bind(pagination.limit.unwrap_or(100))
        .bind(pagination.offset.unwrap_or(0))
        .fetch_all(&self.pool)
        .await
        .context("Failed to list users")?;
        
        Ok(users)
    }

    /// Updates the last login timestamp for a user
    pub async fn update_user_last_login(&self, user_id: Uuid) -> Result<()> {
        let now = Utc::now();
        
        sqlx::query(
            "UPDATE users SET last_login = $1 WHERE id = $2"
        )
        .bind(now)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .context("Failed to update user last login")?;
        
        Ok(())
    }
    
    // === API Endpoint Management ===
    
    /// Registers a new monetizable API endpoint
    pub async fn create_endpoint(&self, owner_id: Uuid, request: CreateEndpointRequest) -> Result<ApiEndpoint> {
        let now = Utc::now();
        let allowed_methods = request.allowed_methods.unwrap_or_else(|| vec!["GET".to_string()]);
        
        let endpoint = sqlx::query_as::<_, ApiEndpoint>(
            r#"
            INSERT INTO api_endpoints (name, description, owner_id, upstream_url, price_per_request,
                                     rate_limit, rate_limit_window, requires_auth, allowed_methods,
                                     request_timeout, retry_attempts, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id, name, description, owner_id, upstream_url, price_per_request, is_active,
                      created_at, updated_at, rate_limit, rate_limit_window, requires_auth,
                      allowed_methods, request_timeout, retry_attempts
            "#
        )
        .bind(&request.name)
        .bind(&request.description)
        .bind(owner_id)
        .bind(&request.upstream_url)
        .bind(&request.price_per_request)
        .bind(request.rate_limit)
        .bind(request.rate_limit_window)
        .bind(request.requires_auth.unwrap_or(true))
        .bind(&allowed_methods)
        .bind(request.request_timeout)
        .bind(request.retry_attempts)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create API endpoint")?;
        
        info!("Created API endpoint: {} (ID: {})", endpoint.name, endpoint.id);
        Ok(endpoint)
    }
    
    /// Retrieves endpoint details by ID
    pub async fn get_endpoint_by_id(&self, endpoint_id: Uuid) -> Result<Option<ApiEndpoint>> {
        let endpoint = sqlx::query_as::<_, ApiEndpoint>(
            r#"
            SELECT id, name, description, owner_id, upstream_url, price_per_request, is_active,
                   created_at, updated_at, rate_limit, rate_limit_window, requires_auth,
                   allowed_methods, request_timeout, retry_attempts
            FROM api_endpoints WHERE id = $1
            "#
        )
        .bind(endpoint_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get endpoint by ID")?;
        
        Ok(endpoint)
    }
    
    /// Finds endpoint by its unique name
    pub async fn get_endpoint_by_name(&self, name: &str) -> Result<Option<ApiEndpoint>> {
        let endpoint = sqlx::query_as::<_, ApiEndpoint>(
            r#"
            SELECT id, name, description, owner_id, upstream_url, price_per_request, is_active,
                   created_at, updated_at, rate_limit, rate_limit_window, requires_auth,
                   allowed_methods, request_timeout, retry_attempts
            FROM api_endpoints WHERE name = $1 AND is_active = true
            "#
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get endpoint by name")?;
        
        Ok(endpoint)
    }
    
    /// Updates endpoint configuration and pricing
    pub async fn update_endpoint(&self, endpoint_id: Uuid, request: UpdateEndpointRequest) -> Result<ApiEndpoint> {
        let now = Utc::now();
        
        let endpoint = sqlx::query_as::<_, ApiEndpoint>(
            r#"
            UPDATE api_endpoints SET
                description = COALESCE($2, description),
                upstream_url = COALESCE($3, upstream_url),
                price_per_request = COALESCE($4, price_per_request),
                is_active = COALESCE($5, is_active),
                rate_limit = COALESCE($6, rate_limit),
                rate_limit_window = COALESCE($7, rate_limit_window),
                requires_auth = COALESCE($8, requires_auth),
                allowed_methods = COALESCE($9, allowed_methods),
                request_timeout = COALESCE($10, request_timeout),
                retry_attempts = COALESCE($11, retry_attempts),
                updated_at = $12
            WHERE id = $1
            RETURNING id, name, description, owner_id, upstream_url, price_per_request, is_active,
                      created_at, updated_at, rate_limit, rate_limit_window, requires_auth,
                      allowed_methods, request_timeout, retry_attempts
            "#
        )
        .bind(endpoint_id)
        .bind(request.description)
        .bind(request.upstream_url)
        .bind(request.price_per_request)
        .bind(request.is_active)
        .bind(request.rate_limit)
        .bind(request.rate_limit_window)
        .bind(request.requires_auth)
        .bind(request.allowed_methods)
        .bind(request.request_timeout)
        .bind(request.retry_attempts)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .context("Failed to update endpoint")?;
        
        Ok(endpoint)
    }
    
    /// Lists endpoints with optional owner filtering and pagination
    pub async fn list_endpoints(&self, owner_id: Option<Uuid>, params: PaginationParams) -> Result<PaginatedResponse<ApiEndpoint>> {
        let limit = params.limit.unwrap_or(50) as i64;
        let page = params.page.unwrap_or(1) as i64;
        let offset = (page - 1) * limit;
        
        let (total_query, endpoints_query) = if let Some(owner_id) = owner_id {
            (
                sqlx::query_scalar("SELECT COUNT(*) FROM api_endpoints WHERE owner_id = $1")
                    .bind(owner_id),
                sqlx::query_as::<_, ApiEndpoint>(
                    r#"
                    SELECT id, name, description, owner_id, upstream_url, price_per_request, is_active,
                           created_at, updated_at, rate_limit, rate_limit_window, requires_auth,
                           allowed_methods, request_timeout, retry_attempts
                    FROM api_endpoints 
                    WHERE owner_id = $1
                    ORDER BY created_at DESC
                    LIMIT $2 OFFSET $3
                    "#
                )
                .bind(owner_id)
                .bind(limit)
                .bind(offset)
            )
        } else {
            (
                sqlx::query_scalar("SELECT COUNT(*) FROM api_endpoints"),
                sqlx::query_as::<_, ApiEndpoint>(
                    r#"
                    SELECT id, name, description, owner_id, upstream_url, price_per_request, is_active,
                           created_at, updated_at, rate_limit, rate_limit_window, requires_auth,
                           allowed_methods, request_timeout, retry_attempts
                    FROM api_endpoints 
                    ORDER BY created_at DESC
                    LIMIT $1 OFFSET $2
                    "#
                )
                .bind(limit)
                .bind(offset)
            )
        };
        
        let total: i64 = total_query
            .fetch_one(&self.pool)
            .await
            .context("Failed to get total endpoint count")?;
            
        let endpoints = endpoints_query
            .fetch_all(&self.pool)
            .await
            .context("Failed to list endpoints")?;
        
        Ok(PaginatedResponse {
            data: endpoints,
            total,
            page: page as u32,
            limit: limit as u32,
            total_pages: ((total + limit - 1) / limit) as u32,
        })
    }
    
    // === Request Logging ===
    
    /// Logs API request details for debugging and analytics
    pub async fn create_request_log(&self, request: CreateRequestLogRequest) -> Result<RequestLog> {
        let now = Utc::now();
        
        let log = sqlx::query_as::<_, RequestLog>(
            r#"
            INSERT INTO request_logs (user_id, endpoint_id, request_id, method, path, status_code,
                                    response_time_ms, request_size, response_size, ip_address_hash,
                                    user_agent_hash, timestamp, cost, error_message)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING id, user_id, endpoint_id, request_id, method, path, status_code,
                      response_time_ms, request_size, response_size, ip_address_hash,
                      user_agent_hash, timestamp, cost, error_message
            "#
        )
        .bind(request.user_id)
        .bind(request.endpoint_id)
        .bind(&request.request_id)
        .bind(&request.method)
        .bind(&request.path)
        .bind(request.status_code)
        .bind(request.response_time_ms)
        .bind(request.request_size)
        .bind(request.response_size)
        .bind(&request.ip_address_hash)
        .bind(&request.user_agent_hash)
        .bind(now)
        .bind(&request.cost)
        .bind(&request.error_message)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create request log")?;
        
        Ok(log)
    }
    
    /// Records API usage for billing purposes
    pub async fn create_usage_record(&self, user_id: Uuid, endpoint_id: Uuid, request_count: i64, total_cost: &str, billing_period: &str) -> Result<UsageRecord> {
        let now = Utc::now();
        
        let record = sqlx::query_as::<_, UsageRecord>(
            r#"
            INSERT INTO usage_records (user_id, endpoint_id, request_count, total_cost, billing_period,
                                     status, timestamp)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, user_id, endpoint_id, request_count, total_cost, billing_period,
                      status, transaction_hash, gas_used, block_number, timestamp
            "#
        )
        .bind(user_id)
        .bind(endpoint_id)
        .bind(request_count)
        .bind(total_cost)
        .bind(billing_period)
        .bind(UsageStatus::Pending)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create usage record")?;
        
        Ok(record)
    }
    
    /// Retrieves usage history for a specific user within date range
    pub async fn get_user_usage(&self, user_id: Uuid, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<Vec<UsageRecord>> {
        let records = sqlx::query_as::<_, UsageRecord>(
            r#"
            SELECT id, user_id, endpoint_id, request_count, total_cost, billing_period,
                   status, transaction_hash, gas_used, block_number, created_at, updated_at
            FROM usage_records 
            WHERE user_id = $1 AND created_at BETWEEN $2 AND $3
            ORDER BY created_at DESC
            "#
        )
        .bind(user_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get user usage")?;
        
        Ok(records)
    }

    // === Analytics ===
    
    /// Calculates total API requests across all endpoints
    pub async fn get_total_requests(&self, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar(
            "SELECT COUNT(*) FROM request_logs WHERE created_at BETWEEN $1 AND $2"
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_one(&self.pool)
        .await
        .context("Failed to get total requests")?;
        
        Ok(count)
    }

    /// Calculates total platform revenue from API usage
    pub async fn get_total_revenue(&self, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<String> {
        let revenue = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total_cost::numeric), 0) FROM usage_records WHERE created_at BETWEEN $1 AND $2"
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_one(&self.pool)
        .await
        .context("Failed to get total revenue")?;
        
        Ok(revenue)
    }

    /// Counts new user registrations in date range
    pub async fn get_new_users(&self, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE created_at BETWEEN $1 AND $2"
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_one(&self.pool)
        .await
        .context("Failed to get new users count")?;
        
        Ok(count)
    }

    /// Counts users who made API calls in date range
    pub async fn get_active_users(&self, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<i64> {
        let count = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT user_id) FROM request_logs WHERE created_at BETWEEN $1 AND $2"
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_one(&self.pool)
        .await
        .context("Failed to get active users count")?;
        
        Ok(count)
    }

    /// Finds users with unpaid usage records for billing
    pub async fn get_users_with_outstanding_usage(&self) -> Result<Vec<User>> {
        let users = sqlx::query_as::<_, User>(
            r#"
            SELECT DISTINCT u.id, u.wallet_address, u.api_key, u.email, u.username, u.is_active, 
                           u.created_at, u.updated_at, u.last_login, u.tier, u.monthly_limit, u.rate_limit_override
            FROM users u
            INNER JOIN usage_records ur ON u.id = ur.user_id
            WHERE ur.status = 'pending'
            "#
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get users with outstanding usage")?;
        
        Ok(users)
    }
    
    /// Retrieves usage statistics for a specific endpoint
    pub async fn get_endpoint_usage(&self, endpoint_id: Uuid, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<Vec<UsageRecord>> {
        let records = sqlx::query_as::<_, UsageRecord>(
            r#"
            SELECT id, user_id, endpoint_id, request_count, total_cost, billing_period,
                   status, transaction_hash, gas_used, block_number, created_at, updated_at
            FROM usage_records 
            WHERE endpoint_id = $1 AND created_at BETWEEN $2 AND $3
            ORDER BY created_at DESC
            "#
        )
        .bind(endpoint_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get endpoint usage")?;
        
        Ok(records)
    }
    
    /// Gets usage records ready for blockchain billing
    pub async fn get_pending_billing(&self, limit: i64) -> Result<Vec<UsageRecord>> {
        let records = sqlx::query_as::<_, UsageRecord>(
            r#"
            SELECT id, user_id, endpoint_id, request_count, total_cost, billing_period,
                   status, transaction_hash, gas_used, block_number, created_at, updated_at
            FROM usage_records 
            WHERE status = 'pending'
            ORDER BY created_at ASC
            LIMIT $1
            "#
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to get pending billing records")?;
        
        Ok(records)
    }
    
    /// Updates usage record status after blockchain transaction
    pub async fn update_usage_status(&self, record_id: Uuid, status: UsageStatus, transaction_hash: Option<&str>, gas_used: Option<&str>, block_number: Option<i64>) -> Result<()> {
        let now = Utc::now();
        
        sqlx::query(
            r#"
            UPDATE usage_records SET
                status = $2,
                transaction_hash = $3,
                gas_used = $4,
                block_number = $5,
                updated_at = $6
            WHERE id = $1
            "#
        )
        .bind(record_id)
        .bind(status)
        .bind(transaction_hash)
        .bind(gas_used)
        .bind(block_number)
        .bind(now)
        .execute(&self.pool)
        .await
        .context("Failed to update usage status")?;
        
        Ok(())
    }
    
    // === Rate Limiting ===
    
    /// Checks current rate limit status for user-endpoint combination
    pub async fn check_rate_limit(&self, user_id: Uuid, endpoint_id: Uuid, window_duration: Duration) -> Result<(i32, i32)> {
        let window_start = Utc::now() - chrono::Duration::from_std(window_duration)
            .context("Invalid window duration")?;
        
        let current_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*) FROM request_logs 
            WHERE user_id = $1 AND endpoint_id = $2 AND created_at >= $3
            "#
        )
        .bind(user_id)
        .bind(endpoint_id)
        .bind(window_start)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check current request count")?;
        
        // Get rate limit from endpoint or user override
        let rate_limit: Option<i32> = sqlx::query_scalar(
            r#"
            SELECT COALESCE(u.rate_limit_override, e.rate_limit) 
            FROM users u, api_endpoints e 
            WHERE u.id = $1 AND e.id = $2
            "#
        )
        .bind(user_id)
        .bind(endpoint_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get rate limit")?;
        
        let limit = rate_limit.unwrap_or(100); // Default rate limit
        Ok((current_count as i32, limit))
    }
    
    // === Daily Statistics ===
    
    /// Retrieves cached daily statistics
    pub async fn get_daily_stats(&self, date: NaiveDate, endpoint_id: Option<Uuid>, user_id: Option<Uuid>) -> Result<Option<DailyStats>> {
        let stats = sqlx::query_as::<_, DailyStats>(
            r#"
            SELECT date, endpoint_id, user_id, request_count, total_revenue, unique_users,
                   avg_response_time, error_rate, created_at, updated_at
            FROM daily_stats 
            WHERE date = $1 AND endpoint_id = $2 AND user_id = $3
            "#
        )
        .bind(date)
        .bind(endpoint_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get daily stats")?;
        
        Ok(stats)
    }
    
    /// Generates and caches daily statistics
    pub async fn create_daily_stats(&self, date: NaiveDate, endpoint_id: Option<Uuid>, user_id: Option<Uuid>) -> Result<DailyStats> {
        let now = Utc::now();
        let (request_count, total_revenue, unique_users, avg_response_time, error_rate) = 
            self.calculate_daily_stats(date, endpoint_id, user_id).await?;
        
        let stats = sqlx::query_as::<_, DailyStats>(
            r#"
            INSERT INTO daily_stats (date, endpoint_id, user_id, request_count, total_revenue,
                                   unique_users, avg_response_time, error_rate, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (date, endpoint_id, user_id) DO UPDATE SET
                request_count = EXCLUDED.request_count,
                total_revenue = EXCLUDED.total_revenue,
                unique_users = EXCLUDED.unique_users,
                avg_response_time = EXCLUDED.avg_response_time,
                error_rate = EXCLUDED.error_rate,
                updated_at = EXCLUDED.updated_at
            RETURNING date, endpoint_id, user_id, request_count, total_revenue, unique_users,
                      avg_response_time, error_rate, created_at, updated_at
            "#
        )
        .bind(date)
        .bind(endpoint_id)
        .bind(user_id)
        .bind(request_count)
        .bind(&total_revenue)
        .bind(unique_users)
        .bind(avg_response_time)
        .bind(error_rate)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create daily stats")?;
        
        Ok(stats)
    }
    
    /// Calculates daily metrics from raw usage data
    async fn calculate_daily_stats(&self, date: NaiveDate, endpoint_id: Option<Uuid>, user_id: Option<Uuid>) -> Result<(i64, String, i32, f64, f64)> {
        let start_of_day = date.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_of_day = date.and_hms_opt(23, 59, 59).unwrap().and_utc();
        
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_requests,
                COALESCE(SUM(cost::numeric), 0)::text as total_cost,
                COUNT(DISTINCT user_id) as unique_users,
                COALESCE(AVG(response_time_ms), 0) as avg_response_time,
                COALESCE(AVG(CASE WHEN status_code >= 400 THEN 1.0 ELSE 0.0 END), 0) as error_rate
            FROM request_logs
            WHERE timestamp BETWEEN $1 AND $2
                AND ($3::uuid IS NULL OR endpoint_id = $3)
                AND ($4::uuid IS NULL OR user_id = $4)
            "#
        )
        .bind(start_of_day)
        .bind(end_of_day)
        .bind(endpoint_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to calculate daily stats")?;
        
        Ok((
            row.get::<i64, _>("total_requests"),
            row.get::<String, _>("total_cost"),
            row.get::<i64, _>("unique_users") as i32,
            row.get::<Option<f64>, _>("avg_response_time").unwrap_or(0.0),
            row.get::<Option<f64>, _>("error_rate").unwrap_or(0.0),
        ))
    }
    
    // === Transaction Management ===
    
    /// Starts a database transaction for atomic operations
    pub async fn begin_transaction(&self) -> Result<Transaction<'_, Postgres>> {
        self.pool.begin().await.context("Failed to begin transaction")
    }
    
    // === Maintenance ===
    
    /// Removes old request logs to manage database size
    pub async fn cleanup_old_logs(&self, days: i32) -> Result<u64> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);
        
        let result = sqlx::query(
            "DELETE FROM request_logs WHERE timestamp < $1"
        )
        .bind(cutoff_date)
        .execute(&self.pool)
        .await
        .context("Failed to cleanup old logs")?;
        
        info!("Cleaned up {} old request logs", result.rows_affected());
        Ok(result.rows_affected())
    }
    
    /// Optimizes database performance with vacuum and analyze
    pub async fn vacuum_analyze(&self) -> Result<()> {
        // Note: VACUUM and ANALYZE cannot be run in a transaction
        sqlx::query("VACUUM ANALYZE")
            .execute(&self.pool)
            .await
            .context("Failed to vacuum analyze database")?;
        
        info!("Database vacuum analyze completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    async fn setup_test_db() -> Database {
        let config = Config::load().unwrap();
        let db = Database::new(&config.database_url, 1).await.unwrap();
        db.migrate().await.unwrap();
        db
    }
    
    #[tokio::test]
    #[ignore] // Requires database connection
    async fn test_user_crud() {
        let db = setup_test_db().await;
        
        let create_request = CreateUserRequest {
            wallet_address: "0x1234567890123456789012345678901234567890".to_string(),
            email: Some("test@example.com".to_string()),
            username: Some("testuser".to_string()),
            tier: Some(UserTier::Free),
        };
        
        let user = db.create_user(create_request).await.unwrap();
        assert_eq!(user.wallet_address, "0x1234567890123456789012345678901234567890");
        assert_eq!(user.email, Some("test@example.com".to_string()));
        
        // Test user retrieval
        let retrieved_user = db.get_user_by_id(user.id).await.unwrap();
        assert!(retrieved_user.is_some());
        assert_eq!(retrieved_user.unwrap().id, user.id);
        
        // Test user update
        let update_request = UpdateUserRequest {
            email: Some("updated@example.com".to_string()),
            username: None,
            is_active: Some(false),
            tier: None,
            monthly_limit: None,
            rate_limit_override: None,
        };
        
        let updated_user = db.update_user(user.id, update_request).await.unwrap();
        assert_eq!(updated_user.email, Some("updated@example.com".to_string()));
        assert!(!updated_user.is_active);
    }
    
    #[tokio::test]
    #[ignore] // Requires database connection
    async fn test_endpoint_crud() {
        let db = setup_test_db().await;
        
        // First create a user to own the endpoint
        let create_user_request = CreateUserRequest {
            wallet_address: "0x1234567890123456789012345678901234567890".to_string(),
            email: Some("owner@example.com".to_string()),
            username: Some("owner".to_string()),
            tier: Some(UserTier::Pro),
        };
        
        let user = db.create_user(create_user_request).await.unwrap();
        
        // Test endpoint creation
        let create_request = CreateEndpointRequest {
            name: "test-api".to_string(),
            description: Some("Test API endpoint".to_string()),
            upstream_url: "https://api.example.com".to_string(),
            price_per_request: "0.001".to_string(),
            rate_limit: Some(1000),
            rate_limit_window: Some(3600),
            requires_auth: Some(true),
            allowed_methods: Some(vec!["GET".to_string(), "POST".to_string()]),
            request_timeout: Some(30),
            retry_attempts: Some(3),
        };
        
        let endpoint = db.create_endpoint(user.id, create_request).await.unwrap();
        assert_eq!(endpoint.name, "test-api");
        assert_eq!(endpoint.owner_id, user.id);
    }
}