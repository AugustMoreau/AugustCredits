//! Metering service for AugustCredits
//!
//! Comprehensive usage tracking and rate limiting system that monitors API consumption,
//! enforces user limits, collects billing metrics, and provides real-time analytics
//! for the monetization platform.

use crate::{
    database::Database,
    error::{AppError, AppResult},
    models::*,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Sliding window rate limiter for tracking request timestamps
#[derive(Debug, Clone)]
struct RateLimitWindow {
    requests: Vec<u64>, // timestamps
    limit: u32,
    window_seconds: u32,
}

impl RateLimitWindow {
    /// Creates a new rate limiting window with specified limits
    fn new(limit: u32, window_seconds: u32) -> Self {
        Self {
            requests: Vec::new(),
            limit,
            window_seconds,
        }
    }

    /// Checks if a new request can be made within rate limits
    fn can_make_request(&mut self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Remove old requests outside the window
        self.requests.retain(|&timestamp| now - timestamp < self.window_seconds as u64);
        
        // Check if we can make another request
        if self.requests.len() < self.limit as usize {
            self.requests.push(now);
            true
        } else {
            false
        }
    }

    /// Returns the number of requests remaining in the current window
    fn remaining_requests(&self) -> u32 {
        self.limit.saturating_sub(self.requests.len() as u32)
    }

    /// Returns the timestamp when the rate limit window resets
    fn reset_time(&self) -> Option<u64> {
        self.requests.first().map(|&first| first + self.window_seconds as u64)
    }
}

/// Core metering service for usage tracking and rate limiting
#[derive(Clone)]
pub struct MeteringService {
    database: Arc<Database>,
    // In-memory rate limiting cache
    rate_limits: Arc<RwLock<HashMap<String, RateLimitWindow>>>,
    // Default rate limits
    default_rate_limit: u32,
    default_window_seconds: u32,
}

impl MeteringService {
    /// Creates a new metering service with default rate limiting configuration
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
            default_rate_limit: 1000, // 1000 requests per hour by default
            default_window_seconds: 3600, // 1 hour
        }
    }

    /// Validates if a user can make a request within their rate limits
    pub async fn check_rate_limit(&self, user_id: Uuid, endpoint_id: Uuid) -> AppResult<()> {
        // Get endpoint configuration
        let endpoint = self.database
            .get_endpoint_by_id(endpoint_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

        // Get user configuration
        let user = self.database
            .get_user_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        // Determine rate limit
        let (limit, window) = self.get_rate_limit(&user, &endpoint);
        
        // Create cache key
        let cache_key = format!("{}:{}", user_id, endpoint_id);
        
        // Check rate limit
        let mut rate_limits = self.rate_limits.write().await;
        let window_entry = rate_limits
            .entry(cache_key.clone())
            .or_insert_with(|| RateLimitWindow::new(limit, window));
        
        if !window_entry.can_make_request() {
            let reset_time = window_entry.reset_time().unwrap_or(0);
            return Err(AppError::RateLimit(format!(
                "Rate limit exceeded. Limit: {} requests per {} seconds. Reset at: {}",
                limit, window, reset_time
            )));
        }

        debug!(
            "Rate limit check passed for user {} on endpoint {} ({} remaining)",
            user_id, endpoint_id, window_entry.remaining_requests()
        );

        Ok(())
    }

    /// Record a request for billing and analytics
    /// Records a completed API request for billing and analytics
    pub async fn record_request(
        &self,
        user_id: Uuid,
        endpoint_id: Uuid,
        status_code: i32,
        response_time_ms: i32,
    ) -> AppResult<()> {
        // Update database rate limit tracking
        let window_duration = Duration::from_secs(self.default_window_seconds as u64);
        
        if let Err(e) = self.database
            .check_rate_limit(user_id, endpoint_id, window_duration)
            .await
        {
            warn!("Failed to update database rate limit: {}", e);
        }

        // Record metrics for analytics
        self.record_metrics(user_id, endpoint_id, status_code, response_time_ms).await?;

        Ok(())
    }

    /// Get rate limit information for a user/endpoint combination
    /// Retrieves current rate limit status for a user-endpoint combination
    pub async fn get_rate_limit_info(
        &self,
        user_id: Uuid,
        endpoint_id: Uuid,
    ) -> AppResult<RateLimitInfo> {
        let endpoint = self.database
            .get_endpoint_by_id(endpoint_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Endpoint not found".to_string()))?;

        let user = self.database
            .get_user_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

        let (limit, window) = self.get_rate_limit(&user, &endpoint);
        let cache_key = format!("{}:{}", user_id, endpoint_id);
        
        let rate_limits = self.rate_limits.read().await;
        let remaining = rate_limits
            .get(&cache_key)
            .map(|w| w.remaining_requests())
            .unwrap_or(limit);
        
        let reset_time = rate_limits
            .get(&cache_key)
            .and_then(|w| w.reset_time())
            .unwrap_or(0);

        Ok(RateLimitInfo {
            limit,
            remaining,
            reset_time,
            window_seconds: window,
        })
    }

    /// Get usage statistics for a user
    /// Gets comprehensive usage statistics for a user over a time period
    pub async fn get_user_usage(
        &self,
        user_id: Uuid,
        period: UsagePeriod,
    ) -> AppResult<UserUsageStats> {
        let (start_date, end_date) = self.get_period_dates(period);
        
        let usage_records = self.database
            .get_user_usage(user_id, start_date, end_date)
            .await?;

        let total_requests: i64 = usage_records.iter().map(|r| r.request_count).sum();
        let total_cost = usage_records
            .iter()
            .map(|r| r.total_cost.parse::<f64>().unwrap_or(0.0))
            .sum::<f64>()
            .to_string();

        Ok(UserUsageStats {
            user_id,
            period: format!("{:?}", period),
            total_requests,
            total_cost,
            unique_endpoints: usage_records.len() as u32,
            start_date,
            end_date,
        })
    }

    /// Retrieves the current balance for a user account
    pub async fn get_user_balance(&self, _user_id: Uuid) -> AppResult<crate::models::UserBalance> {
        // Placeholder implementation
        Err(AppError::Database(anyhow::anyhow!("Not implemented")))
    }

    /// Processes a balance deposit for a user account
    pub async fn deposit_balance(&self, _user_id: Uuid, _payload: crate::models::DepositRequest) -> AppResult<crate::models::DepositResponse> {
        // Placeholder implementation
        Err(AppError::Database(anyhow::anyhow!("Not implemented")))
    }

    /// Processes a balance withdrawal for a user account
    pub async fn withdraw_balance(&self, _user_id: Uuid, _payload: crate::models::WithdrawRequest) -> AppResult<crate::models::WithdrawResponse> {
        // Placeholder implementation
        Err(AppError::Database(anyhow::anyhow!("Not implemented")))
    }

    /// Get usage statistics for an endpoint
    /// Gets usage and revenue statistics for an API endpoint
    pub async fn get_endpoint_usage(
        &self,
        endpoint_id: Uuid,
        period: UsagePeriod,
    ) -> AppResult<EndpointUsageStats> {
        let (start_date, end_date) = self.get_period_dates(period);
        
        let usage_records = self.database
            .get_endpoint_usage(endpoint_id, start_date, end_date)
            .await?;

        let total_requests: i64 = usage_records.iter().map(|r| r.request_count).sum();
        let total_revenue = usage_records
            .iter()
            .map(|r| r.total_cost.parse::<f64>().unwrap_or(0.0))
            .sum::<f64>()
            .to_string();

        Ok(EndpointUsageStats {
            endpoint_id,
            period: format!("{:?}", period),
            total_requests,
            total_revenue,
            unique_users: usage_records.len() as u32,
            start_date,
            end_date,
        })
    }

    /// Clean up old rate limit entries
    /// Removes expired rate limit entries from memory to prevent memory leaks
    pub async fn cleanup_rate_limits(&self) {
        let mut rate_limits = self.rate_limits.write().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        rate_limits.retain(|_, window| {
            window.requests.retain(|&timestamp| now - timestamp < window.window_seconds as u64);
            !window.requests.is_empty()
        });
        
        debug!("Cleaned up rate limit cache, {} entries remaining", rate_limits.len());
    }

    /// Get effective rate limit for a user/endpoint combination
    /// Determines rate limits for a user-endpoint combination based on tier and overrides
    fn get_rate_limit(&self, user: &User, endpoint: &ApiEndpoint) -> (u32, u32) {
        // Priority: user override > endpoint limit > default
        let limit = user.rate_limit_override
            .map(|l| l as u32)
            .or_else(|| endpoint.rate_limit.map(|l| l as u32))
            .unwrap_or(self.default_rate_limit);
        
        let window = endpoint.rate_limit_window
            .map(|w| w as u32)
            .unwrap_or(self.default_window_seconds);
        
        (limit, window)
    }

    /// Record metrics for analytics
    /// Records detailed metrics for analytics and monitoring
    async fn record_metrics(
        &self,
        user_id: Uuid,
        endpoint_id: Uuid,
        status_code: i32,
        response_time_ms: i32,
    ) -> AppResult<()> {
        // This would typically update time-series metrics
        // For now, we'll just log the metrics
        debug!(
            "Recording metrics: user={}, endpoint={}, status={}, response_time={}ms",
            user_id, endpoint_id, status_code, response_time_ms
        );
        
        // In a real implementation, you might:
        // - Update Redis counters
        // - Send to a metrics service like Prometheus
        // - Update database aggregation tables
        
        Ok(())
    }

    /// Get date range for a usage period
    /// Calculates start and end dates for a given usage period
    fn get_period_dates(&self, period: UsagePeriod) -> (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) {
        let now = chrono::Utc::now();
        
        match period {
            UsagePeriod::Hour => {
                let start = now - chrono::Duration::hours(1);
                (start, now)
            }
            UsagePeriod::Day => {
                let start = now - chrono::Duration::days(1);
                (start, now)
            }
            UsagePeriod::Week => {
                let start = now - chrono::Duration::weeks(1);
                (start, now)
            }
            UsagePeriod::Month => {
                let start = now - chrono::Duration::days(30);
                (start, now)
            }
        }
    }
}

/// Rate limit information
/// Current rate limit status information for API responses
#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub limit: u32,
    pub remaining: u32,
    pub reset_time: u64,
    pub window_seconds: u32,
}

/// User usage statistics
/// Comprehensive usage statistics for a user over a time period
#[derive(Debug, Serialize, Deserialize)]
pub struct UserUsageStats {
    pub user_id: Uuid,
    pub period: String,
    pub total_requests: i64,
    pub total_cost: String,
    pub unique_endpoints: u32,
    pub start_date: chrono::DateTime<chrono::Utc>,
    pub end_date: chrono::DateTime<chrono::Utc>,
}

/// Endpoint usage statistics
/// Usage and revenue statistics for an API endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointUsageStats {
    pub endpoint_id: Uuid,
    pub period: String,
    pub total_requests: i64,
    pub total_revenue: String,
    pub unique_users: u32,
    pub start_date: chrono::DateTime<chrono::Utc>,
    pub end_date: chrono::DateTime<chrono::Utc>,
}

/// Usage period enumeration
/// Time periods for usage statistics and analytics
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum UsagePeriod {
    Hour,
    Day,
    Week,
    Month,
}

impl MeteringService {
    /// Processes pending billing records and updates blockchain state
    pub async fn process_billing(&self, db: Arc<Database>) -> Result<()> {
        // In a real implementation, this would be a complex process involving:
        // - Fetching all users with outstanding balances
        // - Calculating usage since last billing cycle
        // - Interacting with the blockchain to process payments
        // - Sending notifications to users
        info!("Processing billing cycle...");

        let users_to_bill = db.get_users_with_outstanding_usage().await?;

        for user in users_to_bill {
            info!("Processing billing for user: {}", user.id);
            // ... billing logic here ...
        }

        info!("Billing cycle completed.");
        Ok(())
    }

    /// Generates comprehensive analytics data for the specified period
    pub async fn get_analytics(&self, db: Arc<Database>, period: UsagePeriod) -> Result<AnalyticsData> {
        info!("Fetching analytics for period: {:?}", period);

        let (start_date, end_date) = self.get_period_dates(period);

        let total_requests = db.get_total_requests(start_date, end_date).await?;
        let total_revenue = db.get_total_revenue(start_date, end_date).await?;
        let new_users = db.get_new_users(start_date, end_date).await?;
        let active_users = db.get_active_users(start_date, end_date).await?;

        Ok(AnalyticsData {
            period: format!("{:?}", period),
            total_requests,
            total_revenue,
            new_users,
            active_users,
            start_date,
            end_date,
        })
    }
}