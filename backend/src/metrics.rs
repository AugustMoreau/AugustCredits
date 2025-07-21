//! Metrics collection and monitoring for AugustCredits
//!
//! Comprehensive metrics system that tracks API usage, performance statistics,
//! billing events, and system health. Provides real-time monitoring data
//! for observability, alerting, and business analytics.

use crate::{
    database::Database,
    error::AppResult,
};
// axum imports removed as they were unused
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tracing::{debug, info};
use uuid::Uuid;

/// Core metrics collection service for tracking application performance
#[derive(Clone)]
pub struct MetricsService {
    database: Arc<Database>,
    // In-memory metrics counters
    counters: Arc<RwLock<HashMap<String, AtomicU64>>>,
    // Request latency tracking
    latencies: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
    // Service start time
    start_time: Instant,
}

impl MetricsService {
    /// Creates a new metrics service with in-memory counters and latency tracking
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            counters: Arc::new(RwLock::new(HashMap::new())),
            latencies: Arc::new(RwLock::new(HashMap::new())),
            start_time: Instant::now(),
        }
    }

    /// Increments a named counter metric by the specified value
    pub async fn increment_counter(&self, name: &str, value: u64) {
        let mut counters = self.counters.write().await;
        let counter = counters
            .entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        counter.fetch_add(value, Ordering::Relaxed);
        
        debug!("Incremented counter '{}' by {}", name, value);
    }

    /// Records a latency measurement for performance tracking
    pub async fn record_latency(&self, name: &str, duration: Duration) {
        let mut latencies = self.latencies.write().await;
        let latency_vec = latencies.entry(name.to_string()).or_insert_with(Vec::new);
        
        // Keep only the last 1000 measurements to prevent memory growth
        if latency_vec.len() >= 1000 {
            latency_vec.remove(0);
        }
        
        latency_vec.push(duration);
        debug!("Recorded latency for '{}': {:?}", name, duration);
    }

    /// Records comprehensive metrics for an API request including timing and data transfer
    pub async fn record_api_request(
        &self,
        endpoint_id: Uuid,
        user_id: Option<Uuid>,
        status_code: u16,
        duration: Duration,
        request_size: u64,
        response_size: u64,
    ) {
        // Increment request counters
        self.increment_counter("api_requests_total", 1).await;
        self.increment_counter(&format!("api_requests_status_{}", status_code), 1).await;
        self.increment_counter(&format!("api_requests_endpoint_{}", endpoint_id), 1).await;
        
        if let Some(uid) = user_id {
            self.increment_counter(&format!("api_requests_user_{}", uid), 1).await;
        }

        // Record latency
        self.record_latency("api_request_duration", duration).await;
        self.record_latency(&format!("api_request_duration_endpoint_{}", endpoint_id), duration).await;

        // Record data transfer metrics
        self.increment_counter("api_request_bytes_sent", request_size).await;
        self.increment_counter("api_response_bytes_sent", response_size).await;

        info!(
            "API request recorded: endpoint={}, user={:?}, status={}, duration={:?}",
            endpoint_id, user_id, status_code, duration
        );
    }

    /// Record billing event metrics
    /// Records billing-related events for financial tracking and analytics
    pub async fn record_billing_event(
        &self,
        event_type: &str,
        user_id: Uuid,
        amount: &str,
        success: bool,
    ) {
        self.increment_counter(&format!("billing_events_{}", event_type), 1).await;
        self.increment_counter(&format!("billing_events_user_{}", user_id), 1).await;
        
        if success {
            self.increment_counter(&format!("billing_events_{}_success", event_type), 1).await;
        } else {
            self.increment_counter(&format!("billing_events_{}_failure", event_type), 1).await;
        }

        debug!(
            "Billing event recorded: type={}, user={}, amount={}, success={}",
            event_type, user_id, amount, success
        );
    }

    /// Record rate limit event
    /// Records rate limiting events to track API abuse and throttling
    pub async fn record_rate_limit_event(&self, user_id: Uuid, endpoint_id: Uuid, blocked: bool) {
        if blocked {
            self.increment_counter("rate_limit_blocks_total", 1).await;
            self.increment_counter(&format!("rate_limit_blocks_user_{}", user_id), 1).await;
            self.increment_counter(&format!("rate_limit_blocks_endpoint_{}", endpoint_id), 1).await;
        }
        
        self.increment_counter("rate_limit_checks_total", 1).await;
    }

    /// Get current metrics snapshot
    /// Creates a snapshot of all current metrics for monitoring systems
    pub async fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        let counters = self.counters.read().await;
        let latencies = self.latencies.read().await;
        
        let mut counter_values = HashMap::new();
        for (name, counter) in counters.iter() {
            counter_values.insert(name.clone(), counter.load(Ordering::Relaxed));
        }
        
        let mut latency_stats = HashMap::new();
        for (name, durations) in latencies.iter() {
            if !durations.is_empty() {
                let stats = calculate_latency_stats(durations);
                latency_stats.insert(name.clone(), stats);
            }
        }
        
        MetricsSnapshot {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            counters: counter_values,
            latencies: latency_stats,
        }
    }

    /// Get system health status
    /// Generates a comprehensive health status report for system monitoring
    pub async fn get_health_status(&self) -> AppResult<HealthStatus> {
        // Check database connectivity
        let db_healthy = self.database.health_check().await.is_ok();
        
        // Check recent error rates
        let counters = self.counters.read().await;
        let total_requests = counters
            .get("api_requests_total")
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0);
        
        let error_requests = counters
            .iter()
            .filter(|(name, _)| name.starts_with("api_requests_status_5"))
            .map(|(_, counter)| counter.load(Ordering::Relaxed))
            .sum::<u64>();
        
        let error_rate = if total_requests > 0 {
            (error_requests as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };
        
        // Determine overall health
        let healthy = db_healthy && error_rate < 5.0; // Less than 5% error rate
        
        Ok(HealthStatus {
            healthy,
            database_healthy: db_healthy,
            error_rate,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            timestamp: chrono::Utc::now(),
        })
    }

    /// Get endpoint-specific metrics
    /// Retrieves performance metrics for a specific API endpoint
    pub async fn get_endpoint_metrics(&self, endpoint_id: Uuid) -> AppResult<EndpointMetrics> {
        let counters = self.counters.read().await;
        let latencies = self.latencies.read().await;
        
        let request_count = counters
            .get(&format!("api_requests_endpoint_{}", endpoint_id))
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0);
        
        let latency_key = format!("api_request_duration_endpoint_{}", endpoint_id);
        let avg_latency_ms = latencies
            .get(&latency_key)
            .map(|durations| {
                if durations.is_empty() {
                    0.0
                } else {
                    let total_ms: f64 = durations.iter().map(|d| d.as_millis() as f64).sum();
                    total_ms / durations.len() as f64
                }
            })
            .unwrap_or(0.0);
        
        Ok(EndpointMetrics {
            endpoint_id,
            request_count,
            avg_latency_ms,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Reset all metrics (useful for testing)
    /// Resets all in-memory metrics counters and latency data
    pub async fn reset_metrics(&self) {
        let mut counters = self.counters.write().await;
        let mut latencies = self.latencies.write().await;
        
        counters.clear();
        latencies.clear();
        
        info!("All metrics have been reset");
    }
}

/// Calculate latency statistics from a vector of durations
/// Calculates statistical metrics from a collection of latency measurements
fn calculate_latency_stats(durations: &[Duration]) -> LatencyStats {
    if durations.is_empty() {
        return LatencyStats {
            count: 0,
            avg_ms: 0.0,
            min_ms: 0.0,
            max_ms: 0.0,
            p50_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
        };
    }
    
    let mut sorted_durations = durations.to_vec();
    sorted_durations.sort();
    
    let count = durations.len();
    let total_ms: f64 = durations.iter().map(|d| d.as_millis() as f64).sum();
    let avg_ms = total_ms / count as f64;
    
    let min_ms = sorted_durations.first().unwrap().as_millis() as f64;
    let max_ms = sorted_durations.last().unwrap().as_millis() as f64;
    
    let p50_ms = percentile(&sorted_durations, 50.0).as_millis() as f64;
    let p95_ms = percentile(&sorted_durations, 95.0).as_millis() as f64;
    let p99_ms = percentile(&sorted_durations, 99.0).as_millis() as f64;
    
    LatencyStats {
        count,
        avg_ms,
        min_ms,
        max_ms,
        p50_ms,
        p95_ms,
        p99_ms,
    }
}

/// Calculate percentile from sorted durations
/// Calculates the specified percentile from sorted duration measurements
fn percentile(sorted_durations: &[Duration], percentile: f64) -> Duration {
    if sorted_durations.is_empty() {
        return Duration::from_millis(0);
    }
    
    let index = ((percentile / 100.0) * (sorted_durations.len() - 1) as f64).round() as usize;
    sorted_durations[index.min(sorted_durations.len() - 1)]
}

/// Metrics snapshot structure
/// Point-in-time snapshot of all system metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub timestamp: u64,
    pub uptime_seconds: u64,
    pub counters: HashMap<String, u64>,
    pub latencies: HashMap<String, LatencyStats>,
}

/// Latency statistics
/// Statistical analysis of latency measurements
#[derive(Debug, Serialize, Deserialize)]
pub struct LatencyStats {
    pub count: usize,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
}

/// Health status structure
/// System health status information for monitoring
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub database_healthy: bool,
    pub error_rate: f64,
    pub uptime_seconds: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Endpoint-specific metrics
/// Performance metrics for a specific API endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct EndpointMetrics {
    pub endpoint_id: Uuid,
    pub request_count: u64,
    pub avg_latency_ms: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Metrics query parameters
/// Query parameters for filtering metrics data
#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    // Currently no query parameters needed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use std::time::Duration;

    /// Tests basic metrics collection functionality
    #[tokio::test]
    async fn test_metrics_collection() {
        let database = Arc::new(Database::new_test().await.unwrap());
        let metrics_service = MetricsService::new(database);
        
        // Test counter increment
        metrics_service.increment_counter("test_counter", 5).await;
        metrics_service.increment_counter("test_counter", 3).await;
        
        // Test latency recording
        metrics_service.record_latency("test_latency", Duration::from_millis(100)).await;
        metrics_service.record_latency("test_latency", Duration::from_millis(200)).await;
        
        // Get snapshot
        let snapshot = metrics_service.get_metrics_snapshot().await;
        
        // Verify counter
        assert_eq!(snapshot.counters.get("test_counter"), Some(&8));
        
        // Verify latency stats
        let latency_stats = snapshot.latencies.get("test_latency").unwrap();
        assert_eq!(latency_stats.count, 2);
        assert_eq!(latency_stats.avg_ms, 150.0);
    }

    /// Tests percentile calculation accuracy
    #[test]
    fn test_percentile_calculation() {
        let durations = vec![
            Duration::from_millis(100),
            Duration::from_millis(200),
            Duration::from_millis(300),
            Duration::from_millis(400),
            Duration::from_millis(500),
        ];
        
        assert_eq!(percentile(&durations, 50.0), Duration::from_millis(300));
        assert_eq!(percentile(&durations, 95.0), Duration::from_millis(500));
    }
}