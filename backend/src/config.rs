//! Configuration management for AugustCredits backend
//!
//! Centralized configuration system that loads settings from environment variables,
//! validates required parameters, and provides sensible defaults for development.
//! Manages blockchain connections, authentication security, rate limiting policies,
//! monitoring settings, and feature toggles.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;

/// Complete application configuration loaded from environment variables
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server_address: String,
    pub database_url: String,
    pub redis_url: String,
    pub blockchain: BlockchainConfig,
    pub auth: AuthConfig,
    pub rate_limiting: RateLimitingConfig,
    pub monitoring: MonitoringConfig,
    pub features: FeatureFlags,
}

/// Blockchain network configuration for smart contract interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub billing_contract_address: String,
    pub metering_contract_address: String,
    pub payments_contract_address: String,
    pub private_key: String,
    pub gas_limit: u64,
    pub gas_price_gwei: u64,
    pub confirmation_blocks: u64,
    pub retry_attempts: u32,
    pub retry_delay_ms: u64,
}

/// Authentication and security settings for user management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiry_hours: u64,
    pub refresh_token_expiry_days: u64,
    pub bcrypt_cost: u32,
    pub api_key_length: usize,
    pub require_email_verification: bool,
    pub max_login_attempts: u32,
    pub lockout_duration_minutes: u64,
}

/// Rate limiting configuration to prevent API abuse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    pub default_requests_per_hour: u32,
    pub default_burst_size: u32,
    pub admin_requests_per_hour: u32,
    pub enable_ip_rate_limiting: bool,
    pub enable_user_rate_limiting: bool,
    pub redis_key_prefix: String,
}

/// Observability and monitoring configuration for system health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enable_metrics: bool,
    pub metrics_port: u16,
    pub log_level: String,
    pub enable_tracing: bool,
    pub jaeger_endpoint: Option<String>,
    pub prometheus_namespace: String,
}

/// Feature flags for enabling experimental or optional functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub enable_escrow: bool,
    pub enable_streaming_payments: bool,
    pub enable_dispute_resolution: bool,
    pub enable_analytics: bool,
    pub enable_webhooks: bool,
    pub enable_batch_billing: bool,
}

impl Config {
    /// Loads and validates configuration from environment variables
    /// 
    /// First attempts to load from .env file for development convenience,
    /// then reads from system environment. Validates all required settings
    /// and returns detailed errors for missing or invalid configuration.
    pub fn load() -> Result<Self> {
        // Try loading from .env file for development convenience
        dotenvy::dotenv().ok();

        let config = Config {
            server_address: env::var("SERVER_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL environment variable is required")?,
            
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            
            blockchain: BlockchainConfig {
                rpc_url: env::var("BLOCKCHAIN_RPC_URL")
                    .context("BLOCKCHAIN_RPC_URL environment variable is required")?,
                
                chain_id: env::var("BLOCKCHAIN_CHAIN_ID")
                    .unwrap_or_else(|_| "1".to_string())
                    .parse()
                    .context("Invalid BLOCKCHAIN_CHAIN_ID")?,
                
                billing_contract_address: env::var("BILLING_CONTRACT_ADDRESS")
                    .context("BILLING_CONTRACT_ADDRESS environment variable is required")?,
                
                metering_contract_address: env::var("METERING_CONTRACT_ADDRESS")
                    .context("METERING_CONTRACT_ADDRESS environment variable is required")?,
                
                payments_contract_address: env::var("PAYMENTS_CONTRACT_ADDRESS")
                    .context("PAYMENTS_CONTRACT_ADDRESS environment variable is required")?,
                
                private_key: env::var("BLOCKCHAIN_PRIVATE_KEY")
                    .context("BLOCKCHAIN_PRIVATE_KEY environment variable is required")?,
                
                gas_limit: env::var("BLOCKCHAIN_GAS_LIMIT")
                    .unwrap_or_else(|_| "3000000".to_string())
                    .parse()
                    .context("Invalid BLOCKCHAIN_GAS_LIMIT")?,
                
                gas_price_gwei: env::var("BLOCKCHAIN_GAS_PRICE_GWEI")
                    .unwrap_or_else(|_| "20".to_string())
                    .parse()
                    .context("Invalid BLOCKCHAIN_GAS_PRICE_GWEI")?,
                
                confirmation_blocks: env::var("BLOCKCHAIN_CONFIRMATION_BLOCKS")
                    .unwrap_or_else(|_| "3".to_string())
                    .parse()
                    .context("Invalid BLOCKCHAIN_CONFIRMATION_BLOCKS")?,
                
                retry_attempts: env::var("BLOCKCHAIN_RETRY_ATTEMPTS")
                    .unwrap_or_else(|_| "3".to_string())
                    .parse()
                    .context("Invalid BLOCKCHAIN_RETRY_ATTEMPTS")?,
                
                retry_delay_ms: env::var("BLOCKCHAIN_RETRY_DELAY_MS")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .context("Invalid BLOCKCHAIN_RETRY_DELAY_MS")?,
            },
            
            auth: AuthConfig {
                jwt_secret: env::var("JWT_SECRET")
                    .context("JWT_SECRET environment variable is required")?,
                
                jwt_expiry_hours: env::var("JWT_EXPIRY_HOURS")
                    .unwrap_or_else(|_| "24".to_string())
                    .parse()
                    .context("Invalid JWT_EXPIRY_HOURS")?,
                
                refresh_token_expiry_days: env::var("REFRESH_TOKEN_EXPIRY_DAYS")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .context("Invalid REFRESH_TOKEN_EXPIRY_DAYS")?,
                
                bcrypt_cost: env::var("BCRYPT_COST")
                    .unwrap_or_else(|_| "12".to_string())
                    .parse()
                    .context("Invalid BCRYPT_COST")?,
                
                api_key_length: env::var("API_KEY_LENGTH")
                    .unwrap_or_else(|_| "32".to_string())
                    .parse()
                    .context("Invalid API_KEY_LENGTH")?,
                
                require_email_verification: env::var("REQUIRE_EMAIL_VERIFICATION")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .context("Invalid REQUIRE_EMAIL_VERIFICATION")?,
                
                max_login_attempts: env::var("MAX_LOGIN_ATTEMPTS")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse()
                    .context("Invalid MAX_LOGIN_ATTEMPTS")?,
                
                lockout_duration_minutes: env::var("LOCKOUT_DURATION_MINUTES")
                    .unwrap_or_else(|_| "15".to_string())
                    .parse()
                    .context("Invalid LOCKOUT_DURATION_MINUTES")?,
            },
            
            rate_limiting: RateLimitingConfig {
                default_requests_per_hour: env::var("DEFAULT_REQUESTS_PER_HOUR")
                    .unwrap_or_else(|_| "1000".to_string())
                    .parse()
                    .context("Invalid DEFAULT_REQUESTS_PER_HOUR")?,
                
                default_burst_size: env::var("DEFAULT_BURST_SIZE")
                    .unwrap_or_else(|_| "100".to_string())
                    .parse()
                    .context("Invalid DEFAULT_BURST_SIZE")?,
                
                admin_requests_per_hour: env::var("ADMIN_REQUESTS_PER_HOUR")
                    .unwrap_or_else(|_| "10000".to_string())
                    .parse()
                    .context("Invalid ADMIN_REQUESTS_PER_HOUR")?,
                
                enable_ip_rate_limiting: env::var("ENABLE_IP_RATE_LIMITING")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .context("Invalid ENABLE_IP_RATE_LIMITING")?,
                
                enable_user_rate_limiting: env::var("ENABLE_USER_RATE_LIMITING")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .context("Invalid ENABLE_USER_RATE_LIMITING")?,
                
                redis_key_prefix: env::var("REDIS_KEY_PREFIX")
                    .unwrap_or_else(|_| "august_credits".to_string()),
            },
            
            monitoring: MonitoringConfig {
                enable_metrics: env::var("ENABLE_METRICS")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .context("Invalid ENABLE_METRICS")?,
                
                metrics_port: env::var("METRICS_PORT")
                    .unwrap_or_else(|_| "9090".to_string())
                    .parse()
                    .context("Invalid METRICS_PORT")?,
                
                log_level: env::var("LOG_LEVEL")
                    .unwrap_or_else(|_| "info".to_string()),
                
                enable_tracing: env::var("ENABLE_TRACING")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .context("Invalid ENABLE_TRACING")?,
                
                jaeger_endpoint: env::var("JAEGER_ENDPOINT").ok(),
                
                prometheus_namespace: env::var("PROMETHEUS_NAMESPACE")
                    .unwrap_or_else(|_| "august_credits".to_string()),
            },
            
            features: FeatureFlags {
                enable_escrow: env::var("ENABLE_ESCROW")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .context("Invalid ENABLE_ESCROW")?,
                
                enable_streaming_payments: env::var("ENABLE_STREAMING_PAYMENTS")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .context("Invalid ENABLE_STREAMING_PAYMENTS")?,
                
                enable_dispute_resolution: env::var("ENABLE_DISPUTE_RESOLUTION")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .context("Invalid ENABLE_DISPUTE_RESOLUTION")?,
                
                enable_analytics: env::var("ENABLE_ANALYTICS")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .context("Invalid ENABLE_ANALYTICS")?,
                
                enable_webhooks: env::var("ENABLE_WEBHOOKS")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .context("Invalid ENABLE_WEBHOOKS")?,
                
                enable_batch_billing: env::var("ENABLE_BATCH_BILLING")
                    .unwrap_or_else(|_| "true".to_string())
                    .parse()
                    .context("Invalid ENABLE_BATCH_BILLING")?,
            },
        };

        // Ensure all configuration values are valid before returning
        config.validate()?;
        
        Ok(config)
    }
    
    /// Validates all configuration values for correctness and security
    fn validate(&self) -> Result<()> {
        // Validate server address
        if self.server_address.is_empty() {
            anyhow::bail!("Server address cannot be empty");
        }
        
        // Validate database URL
        if !self.database_url.starts_with("postgres://") && !self.database_url.starts_with("postgresql://") {
            anyhow::bail!("Database URL must be a valid PostgreSQL connection string");
        }
        
        // Validate blockchain configuration
        if self.blockchain.rpc_url.is_empty() {
            anyhow::bail!("Blockchain RPC URL cannot be empty");
        }
        
        if self.blockchain.billing_contract_address.len() != 42 || !self.blockchain.billing_contract_address.starts_with("0x") {
            anyhow::bail!("Invalid billing contract address format");
        }
        
        if self.blockchain.metering_contract_address.len() != 42 || !self.blockchain.metering_contract_address.starts_with("0x") {
            anyhow::bail!("Invalid metering contract address format");
        }
        
        if self.blockchain.payments_contract_address.len() != 42 || !self.blockchain.payments_contract_address.starts_with("0x") {
            anyhow::bail!("Invalid payments contract address format");
        }
        
        if self.blockchain.private_key.len() != 64 && self.blockchain.private_key.len() != 66 {
            anyhow::bail!("Invalid private key format");
        }
        
        // Validate auth configuration
        if self.auth.jwt_secret.len() < 32 {
            anyhow::bail!("JWT secret must be at least 32 characters long");
        }
        
        if self.auth.bcrypt_cost < 4 || self.auth.bcrypt_cost > 31 {
            anyhow::bail!("Bcrypt cost must be between 4 and 31");
        }
        
        if self.auth.api_key_length < 16 {
            anyhow::bail!("API key length must be at least 16 characters");
        }
        
        // Validate rate limiting
        if self.rate_limiting.default_requests_per_hour == 0 {
            anyhow::bail!("Default requests per hour must be greater than 0");
        }
        
        if self.rate_limiting.default_burst_size == 0 {
            anyhow::bail!("Default burst size must be greater than 0");
        }
        
        // Validate monitoring
        if self.monitoring.metrics_port == 0 {
            anyhow::bail!("Metrics port must be greater than 0");
        }
        
        Ok(())
    }
    
    /// Get the full blockchain RPC URL with authentication if needed
    /// Returns the configured blockchain RPC endpoint URL
    pub fn get_blockchain_rpc_url(&self) -> String {
        self.blockchain.rpc_url.clone()
    }
    
    /// Get the database connection pool configuration
    /// Creates database connection pool configuration from settings
    pub fn get_database_pool_config(&self) -> DatabasePoolConfig {
        DatabasePoolConfig {
            max_connections: 20,
            min_connections: 5,
            acquire_timeout_seconds: 30,
            idle_timeout_seconds: 600,
            max_lifetime_seconds: 3600,
        }
    }
    
    /// Check if a feature is enabled
    /// Checks if a specific feature flag is enabled
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        match feature {
            "escrow" => self.features.enable_escrow,
            "streaming_payments" => self.features.enable_streaming_payments,
            "dispute_resolution" => self.features.enable_dispute_resolution,
            "analytics" => self.features.enable_analytics,
            "webhooks" => self.features.enable_webhooks,
            "batch_billing" => self.features.enable_batch_billing,
            _ => false,
        }
    }
}

/// Database connection pool configuration parameters
#[derive(Debug, Clone)]
pub struct DatabasePoolConfig {
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
    pub max_lifetime_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    
    /// Tests configuration validation with various invalid inputs
    #[test]
    fn test_config_validation() {
        // Set required environment variables for testing
        env::set_var("DATABASE_URL", "postgresql://user:pass@localhost/test");
        env::set_var("BLOCKCHAIN_RPC_URL", "https://mainnet.infura.io/v3/test");
        env::set_var("BILLING_CONTRACT_ADDRESS", "0x1234567890123456789012345678901234567890");
        env::set_var("METERING_CONTRACT_ADDRESS", "0x1234567890123456789012345678901234567890");
        env::set_var("PAYMENTS_CONTRACT_ADDRESS", "0x1234567890123456789012345678901234567890");
        env::set_var("BLOCKCHAIN_PRIVATE_KEY", "1234567890123456789012345678901234567890123456789012345678901234");
        env::set_var("JWT_SECRET", "this_is_a_very_long_jwt_secret_for_testing_purposes_12345");
        
        let config = Config::load();
        assert!(config.is_ok());
    }
    
    /// Tests feature flag checking functionality
    #[test]
    fn test_feature_flags() {
        env::set_var("DATABASE_URL", "postgresql://user:pass@localhost/test");
        env::set_var("BLOCKCHAIN_RPC_URL", "https://mainnet.infura.io/v3/test");
        env::set_var("BILLING_CONTRACT_ADDRESS", "0x1234567890123456789012345678901234567890");
        env::set_var("METERING_CONTRACT_ADDRESS", "0x1234567890123456789012345678901234567890");
        env::set_var("PAYMENTS_CONTRACT_ADDRESS", "0x1234567890123456789012345678901234567890");
        env::set_var("BLOCKCHAIN_PRIVATE_KEY", "1234567890123456789012345678901234567890123456789012345678901234");
        env::set_var("JWT_SECRET", "this_is_a_very_long_jwt_secret_for_testing_purposes_12345");
        env::set_var("ENABLE_ESCROW", "false");
        
        let config = Config::load().unwrap();
        assert!(!config.is_feature_enabled("escrow"));
        assert!(config.is_feature_enabled("analytics")); // Default true
    }
}