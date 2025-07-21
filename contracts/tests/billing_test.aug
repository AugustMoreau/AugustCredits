// AugustCredits Billing Contract Tests

use std::test::TestFramework;
use std::token::MockERC20;
use std::address::Address;

test "user registration works correctly" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let treasury = Address::from("0x1234567890123456789012345678901234567890");
    let billing = AugustCreditsBilling::new(token.address(), treasury);
    
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let api_key = "test-api-key-123";
    
    // Register user
    billing.set_caller(user);
    billing.registerUser(api_key);
    
    // Verify registration
    let user_account = billing.users(user);
    assert(user_account.isActive == true);
    assert(user_account.apiKey == api_key);
    assert(user_account.balance == 0);
    
    // Verify API key mapping
    let mapped_user = billing.apiKeyToUser(api_key);
    assert(mapped_user == user);
}

test "deposit and withdrawal work correctly" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let treasury = Address::from("0x1234567890123456789012345678901234567890");
    let billing = AugustCreditsBilling::new(token.address(), treasury);
    
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let api_key = "test-api-key-123";
    let deposit_amount = 1000000000000000000; // 1 token
    
    // Setup: mint tokens and register user
    token.mint(user, deposit_amount * 2);
    billing.set_caller(user);
    billing.registerUser(api_key);
    
    // Approve and deposit
    token.set_caller(user);
    token.approve(billing.address(), deposit_amount);
    
    billing.set_caller(user);
    billing.depositBalance(deposit_amount);
    
    // Verify deposit
    let balance = billing.getUserBalance(user);
    assert(balance == deposit_amount);
    
    // Test withdrawal
    let withdraw_amount = 500000000000000000; // 0.5 tokens
    billing.withdrawBalance(withdraw_amount);
    
    // Verify withdrawal
    let new_balance = billing.getUserBalance(user);
    assert(new_balance == deposit_amount - withdraw_amount);
    
    let user_token_balance = token.balanceOf(user);
    assert(user_token_balance == deposit_amount + withdraw_amount);
}

test "API endpoint registration and pricing" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let treasury = Address::from("0x1234567890123456789012345678901234567890");
    let billing = AugustCreditsBilling::new(token.address(), treasury);
    
    let endpoint_owner = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let endpoint_name = "test-api-v1";
    let price_per_request = 1000000000000000; // 0.001 tokens
    
    // Register endpoint
    billing.set_caller(endpoint_owner);
    billing.registerApiEndpoint(endpoint_name, price_per_request);
    
    // Verify endpoint registration
    let endpoint = billing.apiEndpoints(endpoint_name);
    assert(endpoint.isActive == true);
    assert(endpoint.pricePerRequest == price_per_request);
    assert(endpoint.owner == endpoint_owner);
    assert(endpoint.totalRequests == 0);
    
    // Test price retrieval
    let retrieved_price = billing.getEndpointPrice(endpoint_name);
    assert(retrieved_price == price_per_request);
    
    // Test pricing update
    let new_price = 2000000000000000; // 0.002 tokens
    billing.updateEndpointPricing(endpoint_name, new_price);
    
    let updated_price = billing.getEndpointPrice(endpoint_name);
    assert(updated_price == new_price);
}

test "usage recording and billing" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let treasury = Address::from("0x1234567890123456789012345678901234567890");
    let billing = AugustCreditsBilling::new(token.address(), treasury);
    
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let endpoint_owner = Address::from("0x1111111111111111111111111111111111111111");
    let api_key = "test-api-key-123";
    let endpoint_name = "test-api-v1";
    let price_per_request = 1000000000000000; // 0.001 tokens
    let deposit_amount = 1000000000000000000; // 1 token
    let request_count = 100;
    
    // Setup: mint tokens, register user and endpoint
    token.mint(user, deposit_amount);
    
    billing.set_caller(user);
    billing.registerUser(api_key);
    
    billing.set_caller(endpoint_owner);
    billing.registerApiEndpoint(endpoint_name, price_per_request);
    
    // Deposit balance
    token.set_caller(user);
    token.approve(billing.address(), deposit_amount);
    
    billing.set_caller(user);
    billing.depositBalance(deposit_amount);
    
    // Record usage (as contract owner)
    billing.set_caller(billing.owner());
    billing.recordUsage(api_key, endpoint_name, request_count);
    
    // Verify billing
    let expected_cost = request_count * price_per_request;
    let platform_fee = (expected_cost * 250) / 10000; // 2.5% platform fee
    let endpoint_payment = expected_cost - platform_fee;
    
    let user_balance = billing.getUserBalance(user);
    assert(user_balance == deposit_amount - expected_cost);
    
    // Verify usage tracking
    let user_usage = billing.getUserUsage(user, endpoint_name);
    assert(user_usage == request_count);
    
    // Verify endpoint stats
    let endpoint = billing.apiEndpoints(endpoint_name);
    assert(endpoint.totalRequests == request_count);
    
    // Verify payments
    let endpoint_owner_balance = token.balanceOf(endpoint_owner);
    assert(endpoint_owner_balance == endpoint_payment);
    
    let treasury_balance = token.balanceOf(treasury);
    assert(treasury_balance == platform_fee);
}

test "insufficient balance handling" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let treasury = Address::from("0x1234567890123456789012345678901234567890");
    let billing = AugustCreditsBilling::new(token.address(), treasury);
    
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let endpoint_owner = Address::from("0x1111111111111111111111111111111111111111");
    let api_key = "test-api-key-123";
    let endpoint_name = "test-api-v1";
    let price_per_request = 1000000000000000; // 0.001 tokens
    let small_deposit = 50000000000000000; // 0.05 tokens
    let large_request_count = 100; // Would cost 0.1 tokens
    
    // Setup
    token.mint(user, small_deposit);
    
    billing.set_caller(user);
    billing.registerUser(api_key);
    
    billing.set_caller(endpoint_owner);
    billing.registerApiEndpoint(endpoint_name, price_per_request);
    
    token.set_caller(user);
    token.approve(billing.address(), small_deposit);
    
    billing.set_caller(user);
    billing.depositBalance(small_deposit);
    
    // Try to record usage that exceeds balance
    billing.set_caller(billing.owner());
    
    // This should fail
    let result = billing.try_recordUsage(api_key, endpoint_name, large_request_count);
    assert(result.is_error());
    
    // Verify no changes occurred
    let user_balance = billing.getUserBalance(user);
    assert(user_balance == small_deposit);
    
    let user_usage = billing.getUserUsage(user, endpoint_name);
    assert(user_usage == 0);
}

test "cost estimation" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let treasury = Address::from("0x1234567890123456789012345678901234567890");
    let billing = AugustCreditsBilling::new(token.address(), treasury);
    
    let endpoint_owner = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let endpoint_name = "test-api-v1";
    let price_per_request = 1000000000000000; // 0.001 tokens
    let request_count = 50;
    
    // Register endpoint
    billing.set_caller(endpoint_owner);
    billing.registerApiEndpoint(endpoint_name, price_per_request);
    
    // Test cost estimation
    let estimated_cost = billing.estimateCost(endpoint_name, request_count);
    let expected_cost = request_count * price_per_request;
    assert(estimated_cost == expected_cost);
}

test "affordability check" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let treasury = Address::from("0x1234567890123456789012345678901234567890");
    let billing = AugustCreditsBilling::new(token.address(), treasury);
    
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let endpoint_owner = Address::from("0x1111111111111111111111111111111111111111");
    let api_key = "test-api-key-123";
    let endpoint_name = "test-api-v1";
    let price_per_request = 1000000000000000; // 0.001 tokens
    let deposit_amount = 100000000000000000; // 0.1 tokens
    
    // Setup
    token.mint(user, deposit_amount);
    
    billing.set_caller(user);
    billing.registerUser(api_key);
    
    billing.set_caller(endpoint_owner);
    billing.registerApiEndpoint(endpoint_name, price_per_request);
    
    token.set_caller(user);
    token.approve(billing.address(), deposit_amount);
    
    billing.set_caller(user);
    billing.depositBalance(deposit_amount);
    
    // Test affordability
    let can_afford_50 = billing.canAffordUsage(user, endpoint_name, 50); // 0.05 tokens
    assert(can_afford_50 == true);
    
    let can_afford_200 = billing.canAffordUsage(user, endpoint_name, 200); // 0.2 tokens
    assert(can_afford_200 == false);
}

test "batch billing" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let treasury = Address::from("0x1234567890123456789012345678901234567890");
    let billing = AugustCreditsBilling::new(token.address(), treasury);
    
    let user1 = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let user2 = Address::from("0x1111111111111111111111111111111111111111");
    let endpoint_owner = Address::from("0x2222222222222222222222222222222222222222");
    let api_key1 = "test-api-key-1";
    let api_key2 = "test-api-key-2";
    let endpoint_name = "test-api-v1";
    let price_per_request = 1000000000000000; // 0.001 tokens
    let deposit_amount = 1000000000000000000; // 1 token
    
    // Setup users and endpoint
    token.mint(user1, deposit_amount);
    token.mint(user2, deposit_amount);
    
    billing.set_caller(user1);
    billing.registerUser(api_key1);
    token.set_caller(user1);
    token.approve(billing.address(), deposit_amount);
    billing.set_caller(user1);
    billing.depositBalance(deposit_amount);
    
    billing.set_caller(user2);
    billing.registerUser(api_key2);
    token.set_caller(user2);
    token.approve(billing.address(), deposit_amount);
    billing.set_caller(user2);
    billing.depositBalance(deposit_amount);
    
    billing.set_caller(endpoint_owner);
    billing.registerApiEndpoint(endpoint_name, price_per_request);
    
    // Prepare batch data
    let users_list = [user1, user2];
    let endpoints = [endpoint_name, endpoint_name];
    let request_counts = [50, 75];
    
    // Execute batch billing
    billing.set_caller(billing.owner());
    billing.batchBilling(users_list, endpoints, request_counts);
    
    // Verify results
    let user1_usage = billing.getUserUsage(user1, endpoint_name);
    assert(user1_usage == 50);
    
    let user2_usage = billing.getUserUsage(user2, endpoint_name);
    assert(user2_usage == 75);
    
    let expected_cost1 = 50 * price_per_request;
    let expected_cost2 = 75 * price_per_request;
    
    let user1_balance = billing.getUserBalance(user1);
    assert(user1_balance == deposit_amount - expected_cost1);
    
    let user2_balance = billing.getUserBalance(user2);
    assert(user2_balance == deposit_amount - expected_cost2);
}