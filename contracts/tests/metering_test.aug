// AugustCredits Metering Contract Tests

use std::test::TestFramework;
use std::address::Address;
use std::time::MockTime;

test "rate limit setting and checking" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint = "test-api-v1";
    let requests_per_period = 100;
    let period_duration = 3600; // 1 hour
    
    // Set rate limit
    metering.set_caller(metering.owner());
    metering.setRateLimit(endpoint, requests_per_period, period_duration);
    
    // Verify rate limit was set
    let rate_limit = metering.rateLimits(endpoint);
    assert(rate_limit.requestsPerPeriod == requests_per_period);
    assert(rate_limit.periodDuration == period_duration);
    assert(rate_limit.isActive == true);
    
    // Test rate limit checking
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let (allowed, remaining, reset_time) = metering.checkRateLimit(user, endpoint);
    
    assert(allowed == true);
    assert(remaining == requests_per_period - 1);
}

test "rate limit enforcement" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint = "test-api-v1";
    let requests_per_period = 5; // Small limit for testing
    let period_duration = 3600;
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    
    // Set rate limit
    metering.set_caller(metering.owner());
    metering.setRateLimit(endpoint, requests_per_period, period_duration);
    
    // Simulate multiple requests
    metering.set_caller(billing_contract);
    
    for i in 0..requests_per_period {
        let request_id = format!("req-{}", i);
        metering.logRequest(
            user,
            endpoint,
            request_id.as_bytes32(),
            100, // response time
            200, // status code
            "ip_hash".as_bytes32()
        );
        
        // Check remaining requests
        let (allowed, remaining, _) = metering.checkRateLimit(user, endpoint);
        if i < requests_per_period - 1 {
            assert(allowed == true);
            assert(remaining == requests_per_period - i - 2);
        }
    }
    
    // Next request should be rate limited
    let (allowed, remaining, _) = metering.checkRateLimit(user, endpoint);
    assert(allowed == false);
    assert(remaining == 0);
}

test "request logging and analytics" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint = "test-api-v1";
    let user1 = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let user2 = Address::from("0x1111111111111111111111111111111111111111");
    let request_id1 = "req-001";
    let request_id2 = "req-002";
    let request_id3 = "req-003";
    
    metering.set_caller(billing_contract);
    
    // Log requests from different users
    metering.logRequest(
        user1,
        endpoint,
        request_id1.as_bytes32(),
        150, // response time
        200, // status code
        "ip_hash_1".as_bytes32()
    );
    
    metering.logRequest(
        user2,
        endpoint,
        request_id2.as_bytes32(),
        200, // response time
        200, // status code
        "ip_hash_2".as_bytes32()
    );
    
    metering.logRequest(
        user1,
        endpoint,
        request_id3.as_bytes32(),
        100, // response time
        404, // error status
        "ip_hash_1".as_bytes32()
    );
    
    // Check endpoint analytics
    let (total_requests, unique_users, avg_response_time, error_rate) = 
        metering.getEndpointStats(endpoint);
    
    assert(total_requests == 3);
    assert(unique_users == 2);
    assert(avg_response_time == 150); // (150 + 200 + 100) / 3
    assert(error_rate == 3333); // 1/3 * 10000 basis points
    
    // Check user analytics
    let (user1_total, user1_first, user1_last) = metering.getUserStats(user1);
    assert(user1_total == 2);
    
    let (user2_total, user2_first, user2_last) = metering.getUserStats(user2);
    assert(user2_total == 1);
    
    // Check user endpoint usage
    let user1_endpoint_usage = metering.getUserEndpointUsage(user1, endpoint);
    assert(user1_endpoint_usage == 2);
    
    let user2_endpoint_usage = metering.getUserEndpointUsage(user2, endpoint);
    assert(user2_endpoint_usage == 1);
}

test "request log retrieval" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint = "test-api-v1";
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let request_id = "req-retrieve-001";
    let response_time = 250;
    let status_code = 201;
    
    metering.set_caller(billing_contract);
    
    // Log a request
    metering.logRequest(
        user,
        endpoint,
        request_id.as_bytes32(),
        response_time,
        status_code,
        "ip_hash".as_bytes32()
    );
    
    // Retrieve the log
    let (log_user, log_endpoint, log_timestamp, log_response_time, log_status_code) = 
        metering.getRequestLog(request_id.as_bytes32());
    
    assert(log_user == user);
    assert(log_endpoint == endpoint);
    assert(log_response_time == response_time);
    assert(log_status_code == status_code);
}

test "recent logs retrieval" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint = "test-api-v1";
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    
    metering.set_caller(billing_contract);
    
    // Log multiple requests
    for i in 0..5 {
        let request_id = format!("req-recent-{}", i);
        metering.logRequest(
            user,
            endpoint,
            request_id.as_bytes32(),
            100 + i * 10, // varying response times
            200,
            "ip_hash".as_bytes32()
        );
    }
    
    // Retrieve recent logs
    let recent_logs = metering.getRecentLogs(3, 0);
    assert(recent_logs.length == 3);
    
    // Should be in reverse chronological order (most recent first)
    assert(recent_logs[0].responseTime == 140); // Last logged (i=4)
    assert(recent_logs[1].responseTime == 130); // Second to last (i=3)
    assert(recent_logs[2].responseTime == 120); // Third to last (i=2)
}

test "daily usage tracking" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint = "test-api-v1";
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    
    // Mock a specific day timestamp
    let day_timestamp = 1640995200; // 2022-01-01 00:00:00 UTC
    let day_start = day_timestamp / 86400 * 86400;
    
    metering.set_caller(billing_contract);
    
    // Log multiple requests for the same day
    for i in 0..3 {
        let request_id = format!("req-daily-{}", i);
        // Mock timestamp within the same day
        MockTime::set_timestamp(day_timestamp + i * 3600); // Different hours same day
        
        metering.logRequest(
            user,
            endpoint,
            request_id.as_bytes32(),
            100,
            200,
            "ip_hash".as_bytes32()
        );
    }
    
    // Check daily usage for endpoint
    let endpoint_daily_usage = metering.getDailyUsage(endpoint, day_start);
    assert(endpoint_daily_usage == 3);
    
    // Check daily usage for user
    let user_daily_usage = metering.getUserDailyUsage(user, day_start);
    assert(user_daily_usage == 3);
}

test "batch analytics retrieval" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint1 = "api-v1";
    let endpoint2 = "api-v2";
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    
    metering.set_caller(billing_contract);
    
    // Log requests for different endpoints
    metering.logRequest(
        user,
        endpoint1,
        "req-1".as_bytes32(),
        100,
        200,
        "ip_hash".as_bytes32()
    );
    
    metering.logRequest(
        user,
        endpoint1,
        "req-2".as_bytes32(),
        200,
        500, // error
        "ip_hash".as_bytes32()
    );
    
    metering.logRequest(
        user,
        endpoint2,
        "req-3".as_bytes32(),
        150,
        200,
        "ip_hash".as_bytes32()
    );
    
    // Get batch analytics
    let endpoints = [endpoint1, endpoint2];
    let (total_requests, unique_users, avg_response_times, error_rates) = 
        metering.getEndpointAnalyticsBatch(endpoints);
    
    assert(total_requests[0] == 2); // endpoint1
    assert(total_requests[1] == 1); // endpoint2
    assert(unique_users[0] == 1);
    assert(unique_users[1] == 1);
    assert(avg_response_times[0] == 150); // (100 + 200) / 2
    assert(avg_response_times[1] == 150);
    assert(error_rates[0] == 5000); // 1/2 * 10000 basis points
    assert(error_rates[1] == 0); // no errors
}

test "default rate limit usage" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint = "unregistered-api";
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    
    // Check rate limit for endpoint without explicit rate limit
    let (allowed, remaining, _) = metering.checkRateLimit(user, endpoint);
    
    assert(allowed == true);
    assert(remaining == 999); // default 1000 - 1
}

test "rate limit pause and unpause" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint = "test-api-v1";
    let requests_per_period = 10;
    let period_duration = 3600;
    
    metering.set_caller(metering.owner());
    
    // Set and verify rate limit
    metering.setRateLimit(endpoint, requests_per_period, period_duration);
    let rate_limit = metering.rateLimits(endpoint);
    assert(rate_limit.isActive == true);
    
    // Pause rate limit
    metering.pauseRateLimit(endpoint);
    let paused_rate_limit = metering.rateLimits(endpoint);
    assert(paused_rate_limit.isActive == false);
    
    // Unpause rate limit
    metering.unpauseRateLimit(endpoint);
    let unpaused_rate_limit = metering.rateLimits(endpoint);
    assert(unpaused_rate_limit.isActive == true);
}

test "billing contract authorization" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let new_billing_contract = Address::from("0x2222222222222222222222222222222222222222");
    let unauthorized_caller = Address::from("0x3333333333333333333333333333333333333333");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint = "test-api-v1";
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    
    // Authorized billing contract should be able to log requests
    metering.set_caller(billing_contract);
    metering.logRequest(
        user,
        endpoint,
        "req-authorized".as_bytes32(),
        100,
        200,
        "ip_hash".as_bytes32()
    );
    
    // Unauthorized caller should fail
    metering.set_caller(unauthorized_caller);
    let result = metering.try_logRequest(
        user,
        endpoint,
        "req-unauthorized".as_bytes32(),
        100,
        200,
        "ip_hash".as_bytes32()
    );
    assert(result.is_error());
    
    // Update billing contract
    metering.set_caller(metering.owner());
    metering.setBillingContract(new_billing_contract);
    
    // New billing contract should work
    metering.set_caller(new_billing_contract);
    metering.logRequest(
        user,
        endpoint,
        "req-new-billing".as_bytes32(),
        100,
        200,
        "ip_hash".as_bytes32()
    );
    
    // Old billing contract should no longer work
    metering.set_caller(billing_contract);
    let old_result = metering.try_logRequest(
        user,
        endpoint,
        "req-old-billing".as_bytes32(),
        100,
        200,
        "ip_hash".as_bytes32()
    );
    assert(old_result.is_error());
}

test "default rate limit updates" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let new_default_limit = 2000;
    let new_default_period = 7200; // 2 hours
    
    // Update default rate limits
    metering.set_caller(metering.owner());
    metering.updateDefaultRateLimit(new_default_limit, new_default_period);
    
    // Verify updates
    assert(metering.defaultRateLimit() == new_default_limit);
    assert(metering.defaultRatePeriod() == new_default_period);
    
    // Test that new defaults are used for unregistered endpoints
    let endpoint = "new-endpoint";
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    
    let (allowed, remaining, _) = metering.checkRateLimit(user, endpoint);
    assert(allowed == true);
    assert(remaining == new_default_limit - 1);
}

test "log cleanup functionality" {
    let billing_contract = Address::from("0x1111111111111111111111111111111111111111");
    let metering = AugustCreditsMetering::new(billing_contract);
    
    let endpoint = "test-api-v1";
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    
    metering.set_caller(billing_contract);
    
    // Log some requests with different timestamps
    let old_timestamp = 1640995200; // 2022-01-01
    let new_timestamp = 1672531200; // 2023-01-01
    let cleanup_threshold = 1656633600; // 2022-07-01 (between old and new)
    
    // Log old request
    MockTime::set_timestamp(old_timestamp);
    metering.logRequest(
        user,
        endpoint,
        "req-old".as_bytes32(),
        100,
        200,
        "ip_hash".as_bytes32()
    );
    
    // Log new request
    MockTime::set_timestamp(new_timestamp);
    metering.logRequest(
        user,
        endpoint,
        "req-new".as_bytes32(),
        100,
        200,
        "ip_hash".as_bytes32()
    );
    
    // Verify both logs exist
    let logs_before = metering.getRecentLogs(10, 0);
    assert(logs_before.length == 2);
    
    // Cleanup old logs
    metering.set_caller(metering.owner());
    metering.cleanupOldLogs(cleanup_threshold);
    
    // Verify only new log remains
    let logs_after = metering.getRecentLogs(10, 0);
    assert(logs_after.length == 1);
    assert(logs_after[0].timestamp >= cleanup_threshold);
}