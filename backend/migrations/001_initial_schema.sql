-- Initial database schema for AugustCredits
-- Creates all necessary tables, types, and indexes

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create custom types
CREATE TYPE user_tier AS ENUM ('free', 'pro', 'enterprise', 'admin');
CREATE TYPE usage_status AS ENUM ('pending', 'billed', 'failed', 'refunded');
CREATE TYPE billing_status AS ENUM ('pending', 'processing', 'completed', 'failed', 'cancelled');
CREATE TYPE transaction_type AS ENUM ('deposit', 'withdrawal', 'payment', 'refund', 'fee');
CREATE TYPE transaction_status AS ENUM ('pending', 'confirmed', 'failed', 'cancelled');
CREATE TYPE webhook_status AS ENUM ('pending', 'delivered', 'failed', 'cancelled');

-- Users table
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wallet_address VARCHAR(42) NOT NULL UNIQUE,
    api_key VARCHAR(255) NOT NULL UNIQUE,
    email VARCHAR(255),
    username VARCHAR(100),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login TIMESTAMPTZ,
    tier user_tier NOT NULL DEFAULT 'free',
    monthly_limit BIGINT,
    rate_limit_override INTEGER
);

-- API endpoints table
CREATE TABLE api_endpoints (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    upstream_url TEXT NOT NULL,
    price_per_request TEXT NOT NULL, -- Stored as string to handle large numbers
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    rate_limit INTEGER,
    rate_limit_window INTEGER, -- seconds
    requires_auth BOOLEAN NOT NULL DEFAULT true,
    allowed_methods TEXT[] NOT NULL DEFAULT '{"GET"}',
    request_timeout INTEGER, -- seconds
    retry_attempts INTEGER
);

-- Request logs table
CREATE TABLE request_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint_id UUID NOT NULL REFERENCES api_endpoints(id) ON DELETE CASCADE,
    request_id VARCHAR(255) NOT NULL,
    method VARCHAR(10) NOT NULL,
    path TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    response_time_ms INTEGER NOT NULL,
    request_size BIGINT,
    response_size BIGINT,
    ip_address_hash VARCHAR(64) NOT NULL,
    user_agent_hash VARCHAR(64),
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    cost TEXT NOT NULL,
    error_message TEXT
);

-- Usage records table (aggregated by billing period)
CREATE TABLE usage_records (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint_id UUID NOT NULL REFERENCES api_endpoints(id) ON DELETE CASCADE,
    request_count BIGINT NOT NULL DEFAULT 0,
    total_cost TEXT NOT NULL DEFAULT '0',
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    billing_period VARCHAR(7) NOT NULL, -- YYYY-MM format
    status usage_status NOT NULL DEFAULT 'pending',
    transaction_hash VARCHAR(66),
    gas_used TEXT,
    block_number BIGINT,
    UNIQUE(user_id, endpoint_id, billing_period)
);

-- Billing records table
CREATE TABLE billing_records (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    billing_period VARCHAR(7) NOT NULL, -- YYYY-MM format
    total_requests BIGINT NOT NULL DEFAULT 0,
    total_cost TEXT NOT NULL DEFAULT '0',
    status billing_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    processed_at TIMESTAMPTZ,
    transaction_hash VARCHAR(66),
    gas_used TEXT,
    block_number BIGINT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    UNIQUE(user_id, billing_period)
);

-- Payment transactions table
CREATE TABLE payment_transactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    transaction_type transaction_type NOT NULL,
    amount TEXT NOT NULL,
    status transaction_status NOT NULL DEFAULT 'pending',
    transaction_hash VARCHAR(66),
    block_number BIGINT,
    gas_used TEXT,
    gas_price TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    confirmed_at TIMESTAMPTZ,
    retry_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    metadata JSONB
);

-- Daily statistics table
CREATE TABLE daily_stats (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    date DATE NOT NULL,
    endpoint_id UUID REFERENCES api_endpoints(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    total_requests BIGINT NOT NULL DEFAULT 0,
    total_cost TEXT NOT NULL DEFAULT '0',
    unique_users INTEGER NOT NULL DEFAULT 0,
    avg_response_time DOUBLE PRECISION NOT NULL DEFAULT 0,
    error_rate DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(date, endpoint_id, user_id)
);

-- Rate limiting entries table
CREATE TABLE rate_limit_entries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    endpoint_id UUID NOT NULL REFERENCES api_endpoints(id) ON DELETE CASCADE,
    window_start TIMESTAMPTZ NOT NULL,
    request_count INTEGER NOT NULL DEFAULT 0,
    limit_exceeded BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- API keys table
CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key_hash VARCHAR(255) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    permissions TEXT[] NOT NULL DEFAULT '{}',
    is_active BOOLEAN NOT NULL DEFAULT true,
    expires_at TIMESTAMPTZ,
    last_used TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    usage_count BIGINT NOT NULL DEFAULT 0,
    rate_limit_override INTEGER
);

-- System configuration table
CREATE TABLE system_config (
    key VARCHAR(255) PRIMARY KEY,
    value TEXT NOT NULL,
    description TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by UUID NOT NULL REFERENCES users(id)
);

-- Webhook endpoints table
CREATE TABLE webhook_endpoints (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    events TEXT[] NOT NULL DEFAULT '{}',
    secret VARCHAR(255) NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_triggered TIMESTAMPTZ,
    failure_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3
);

-- Webhook deliveries table
CREATE TABLE webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    webhook_id UUID NOT NULL REFERENCES webhook_endpoints(id) ON DELETE CASCADE,
    event_type VARCHAR(255) NOT NULL,
    payload JSONB NOT NULL,
    status webhook_status NOT NULL DEFAULT 'pending',
    response_code INTEGER,
    response_body TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at TIMESTAMPTZ,
    retry_count INTEGER NOT NULL DEFAULT 0,
    next_retry TIMESTAMPTZ
);

-- Create indexes for performance

-- Users indexes
CREATE INDEX idx_users_wallet_address ON users(wallet_address);
CREATE INDEX idx_users_api_key ON users(api_key);
CREATE INDEX idx_users_tier ON users(tier);
CREATE INDEX idx_users_is_active ON users(is_active);
CREATE INDEX idx_users_created_at ON users(created_at);

-- API endpoints indexes
CREATE INDEX idx_api_endpoints_name ON api_endpoints(name);
CREATE INDEX idx_api_endpoints_owner_id ON api_endpoints(owner_id);
CREATE INDEX idx_api_endpoints_is_active ON api_endpoints(is_active);
CREATE INDEX idx_api_endpoints_created_at ON api_endpoints(created_at);

-- Request logs indexes
CREATE INDEX idx_request_logs_user_id ON request_logs(user_id);
CREATE INDEX idx_request_logs_endpoint_id ON request_logs(endpoint_id);
CREATE INDEX idx_request_logs_timestamp ON request_logs(timestamp);
CREATE INDEX idx_request_logs_status_code ON request_logs(status_code);
CREATE INDEX idx_request_logs_user_endpoint_timestamp ON request_logs(user_id, endpoint_id, timestamp);

-- Usage records indexes
CREATE INDEX idx_usage_records_user_id ON usage_records(user_id);
CREATE INDEX idx_usage_records_endpoint_id ON usage_records(endpoint_id);
CREATE INDEX idx_usage_records_billing_period ON usage_records(billing_period);
CREATE INDEX idx_usage_records_status ON usage_records(status);
CREATE INDEX idx_usage_records_timestamp ON usage_records(timestamp);

-- Billing records indexes
CREATE INDEX idx_billing_records_user_id ON billing_records(user_id);
CREATE INDEX idx_billing_records_billing_period ON billing_records(billing_period);
CREATE INDEX idx_billing_records_status ON billing_records(status);
CREATE INDEX idx_billing_records_created_at ON billing_records(created_at);

-- Payment transactions indexes
CREATE INDEX idx_payment_transactions_user_id ON payment_transactions(user_id);
CREATE INDEX idx_payment_transactions_type ON payment_transactions(transaction_type);
CREATE INDEX idx_payment_transactions_status ON payment_transactions(status);
CREATE INDEX idx_payment_transactions_created_at ON payment_transactions(created_at);
CREATE INDEX idx_payment_transactions_hash ON payment_transactions(transaction_hash);

-- Daily stats indexes
CREATE INDEX idx_daily_stats_date ON daily_stats(date);
CREATE INDEX idx_daily_stats_endpoint_id ON daily_stats(endpoint_id);
CREATE INDEX idx_daily_stats_user_id ON daily_stats(user_id);
CREATE INDEX idx_daily_stats_date_endpoint ON daily_stats(date, endpoint_id);

-- Rate limit entries indexes
CREATE INDEX idx_rate_limit_entries_user_endpoint ON rate_limit_entries(user_id, endpoint_id);
CREATE INDEX idx_rate_limit_entries_window_start ON rate_limit_entries(window_start);
CREATE INDEX idx_rate_limit_entries_updated_at ON rate_limit_entries(updated_at);

-- API keys indexes
CREATE INDEX idx_api_keys_user_id ON api_keys(user_id);
CREATE INDEX idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_is_active ON api_keys(is_active);
CREATE INDEX idx_api_keys_expires_at ON api_keys(expires_at);

-- Webhook endpoints indexes
CREATE INDEX idx_webhook_endpoints_user_id ON webhook_endpoints(user_id);
CREATE INDEX idx_webhook_endpoints_is_active ON webhook_endpoints(is_active);

-- Webhook deliveries indexes
CREATE INDEX idx_webhook_deliveries_webhook_id ON webhook_deliveries(webhook_id);
CREATE INDEX idx_webhook_deliveries_status ON webhook_deliveries(status);
CREATE INDEX idx_webhook_deliveries_created_at ON webhook_deliveries(created_at);
CREATE INDEX idx_webhook_deliveries_next_retry ON webhook_deliveries(next_retry);

-- Create functions for automatic timestamp updates
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create triggers for automatic timestamp updates
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_api_endpoints_updated_at BEFORE UPDATE ON api_endpoints
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_rate_limit_entries_updated_at BEFORE UPDATE ON rate_limit_entries
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_system_config_updated_at BEFORE UPDATE ON system_config
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert default system configuration
INSERT INTO system_config (key, value, description, updated_by) VALUES
('platform_fee_percentage', '2.5', 'Platform fee percentage (2.5%)', '00000000-0000-0000-0000-000000000000'),
('min_billing_amount', '1000000000000000', 'Minimum billing amount in wei (0.001 ETH)', '00000000-0000-0000-0000-000000000000'),
('max_request_size', '10485760', 'Maximum request size in bytes (10MB)', '00000000-0000-0000-0000-000000000000'),
('max_response_size', '52428800', 'Maximum response size in bytes (50MB)', '00000000-0000-0000-0000-000000000000'),
('default_rate_limit', '1000', 'Default rate limit per minute', '00000000-0000-0000-0000-000000000000'),
('billing_frequency_hours', '24', 'Billing frequency in hours', '00000000-0000-0000-0000-000000000000'),
('log_retention_days', '90', 'Request log retention period in days', '00000000-0000-0000-0000-000000000000');

-- Create a view for user statistics
CREATE VIEW user_stats AS
SELECT 
    u.id,
    u.wallet_address,
    u.tier,
    u.is_active,
    u.created_at,
    COALESCE(SUM(ur.request_count), 0) as total_requests,
    COALESCE(SUM(ur.total_cost::numeric), 0) as total_spent,
    COUNT(DISTINCT ur.endpoint_id) as endpoints_used,
    MAX(ur.timestamp) as last_request_time
FROM users u
LEFT JOIN usage_records ur ON u.id = ur.user_id
GROUP BY u.id, u.wallet_address, u.tier, u.is_active, u.created_at;

-- Create a view for endpoint statistics
CREATE VIEW endpoint_stats AS
SELECT 
    e.id,
    e.name,
    e.owner_id,
    e.is_active,
    e.created_at,
    COALESCE(SUM(ur.request_count), 0) as total_requests,
    COALESCE(SUM(ur.total_cost::numeric), 0) as total_revenue,
    COUNT(DISTINCT ur.user_id) as unique_users,
    MAX(ur.timestamp) as last_request_time
FROM api_endpoints e
LEFT JOIN usage_records ur ON e.id = ur.endpoint_id
GROUP BY e.id, e.name, e.owner_id, e.is_active, e.created_at;

-- Create a function to clean up old request logs
CREATE OR REPLACE FUNCTION cleanup_old_request_logs(retention_days INTEGER DEFAULT 90)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM request_logs 
    WHERE timestamp < NOW() - INTERVAL '1 day' * retention_days;
    
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Create a function to aggregate daily statistics
CREATE OR REPLACE FUNCTION aggregate_daily_stats(target_date DATE DEFAULT CURRENT_DATE - INTERVAL '1 day')
RETURNS INTEGER AS $$
DECLARE
    inserted_count INTEGER := 0;
BEGIN
    -- Aggregate by endpoint
    INSERT INTO daily_stats (date, endpoint_id, total_requests, total_cost, unique_users, avg_response_time, error_rate)
    SELECT 
        target_date,
        rl.endpoint_id,
        COUNT(*) as total_requests,
        COALESCE(SUM(rl.cost::numeric), 0)::text as total_cost,
        COUNT(DISTINCT rl.user_id) as unique_users,
        AVG(rl.response_time_ms) as avg_response_time,
        AVG(CASE WHEN rl.status_code >= 400 THEN 1.0 ELSE 0.0 END) as error_rate
    FROM request_logs rl
    WHERE DATE(rl.timestamp) = target_date
    GROUP BY rl.endpoint_id
    ON CONFLICT (date, endpoint_id, user_id) DO UPDATE SET
        total_requests = EXCLUDED.total_requests,
        total_cost = EXCLUDED.total_cost,
        unique_users = EXCLUDED.unique_users,
        avg_response_time = EXCLUDED.avg_response_time,
        error_rate = EXCLUDED.error_rate,
        created_at = NOW();
    
    GET DIAGNOSTICS inserted_count = ROW_COUNT;
    
    -- Aggregate by user
    INSERT INTO daily_stats (date, user_id, total_requests, total_cost, unique_users, avg_response_time, error_rate)
    SELECT 
        target_date,
        rl.user_id,
        COUNT(*) as total_requests,
        COALESCE(SUM(rl.cost::numeric), 0)::text as total_cost,
        1 as unique_users, -- Always 1 for user-specific stats
        AVG(rl.response_time_ms) as avg_response_time,
        AVG(CASE WHEN rl.status_code >= 400 THEN 1.0 ELSE 0.0 END) as error_rate
    FROM request_logs rl
    WHERE DATE(rl.timestamp) = target_date
    GROUP BY rl.user_id
    ON CONFLICT (date, endpoint_id, user_id) DO UPDATE SET
        total_requests = EXCLUDED.total_requests,
        total_cost = EXCLUDED.total_cost,
        unique_users = EXCLUDED.unique_users,
        avg_response_time = EXCLUDED.avg_response_time,
        error_rate = EXCLUDED.error_rate,
        created_at = NOW();
    
    GET DIAGNOSTICS inserted_count = inserted_count + ROW_COUNT;
    
    RETURN inserted_count;
END;
$$ LANGUAGE plpgsql;

-- Create indexes for the views (PostgreSQL will use these automatically)
CREATE INDEX idx_usage_records_user_endpoint_time ON usage_records(user_id, endpoint_id, timestamp);
CREATE INDEX idx_request_logs_date ON request_logs(DATE(timestamp));

COMMIT;