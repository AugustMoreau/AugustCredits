// AugustCredits Payments Contract Tests

use std::test::TestFramework;
use std::token::MockERC20;
use std::address::Address;

test "deposit and withdrawal work correctly" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let payments = AugustPayments::new(token.address());
    
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let deposit_amount = 1000000000000000000; // 1 token
    
    // Setup: mint tokens
    token.mint(user, deposit_amount * 2);
    
    // Approve and deposit
    token.set_caller(user);
    token.approve(payments.address(), deposit_amount);
    
    payments.set_caller(user);
    payments.deposit(deposit_amount);
    
    // Verify deposit
    let balance = payments.getBalance(user);
    assert(balance == deposit_amount);
    
    // Test withdrawal
    let withdraw_amount = 500000000000000000; // 0.5 tokens
    payments.withdraw(withdraw_amount);
    
    // Verify withdrawal
    let new_balance = payments.getBalance(user);
    assert(new_balance == deposit_amount - withdraw_amount);
    
    let user_token_balance = token.balanceOf(user);
    assert(user_token_balance == deposit_amount + withdraw_amount);
}

test "escrow creation and release" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let payments = AugustPayments::new(token.address());
    
    let payer = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let payee = Address::from("0x1111111111111111111111111111111111111111");
    let amount = 1000000000000000000; // 1 token
    let escrow_id = "escrow-123";
    
    // Setup: mint tokens and deposit
    token.mint(payer, amount * 2);
    token.set_caller(payer);
    token.approve(payments.address(), amount * 2);
    
    payments.set_caller(payer);
    payments.deposit(amount * 2);
    
    // Create escrow
    payments.createEscrow(escrow_id, payee, amount);
    
    // Verify escrow creation
    let escrow = payments.getEscrowDetails(escrow_id);
    assert(escrow.payer == payer);
    assert(escrow.payee == payee);
    assert(escrow.amount == amount);
    assert(escrow.isActive == true);
    assert(escrow.isReleased == false);
    
    // Verify payer balance reduced
    let payer_balance = payments.getBalance(payer);
    assert(payer_balance == amount); // 2 * amount - amount
    
    // Release escrow
    payments.releaseEscrow(escrow_id);
    
    // Verify escrow released
    let released_escrow = payments.getEscrowDetails(escrow_id);
    assert(released_escrow.isReleased == true);
    assert(released_escrow.isActive == false);
    
    // Verify payee received funds
    let payee_balance = payments.getBalance(payee);
    assert(payee_balance == amount);
}

test "escrow refund" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let payments = AugustPayments::new(token.address());
    
    let payer = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let payee = Address::from("0x1111111111111111111111111111111111111111");
    let amount = 1000000000000000000; // 1 token
    let escrow_id = "escrow-refund-123";
    
    // Setup
    token.mint(payer, amount * 2);
    token.set_caller(payer);
    token.approve(payments.address(), amount * 2);
    
    payments.set_caller(payer);
    payments.deposit(amount * 2);
    
    // Create escrow
    payments.createEscrow(escrow_id, payee, amount);
    
    let initial_payer_balance = payments.getBalance(payer);
    
    // Refund escrow
    payments.refundEscrow(escrow_id);
    
    // Verify escrow refunded
    let refunded_escrow = payments.getEscrowDetails(escrow_id);
    assert(refunded_escrow.isActive == false);
    assert(refunded_escrow.isReleased == false);
    
    // Verify payer got funds back
    let final_payer_balance = payments.getBalance(payer);
    assert(final_payer_balance == initial_payer_balance + amount);
    
    // Verify payee didn't receive funds
    let payee_balance = payments.getBalance(payee);
    assert(payee_balance == 0);
}

test "single payment processing" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let payments = AugustPayments::new(token.address());
    
    let payer = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let payee = Address::from("0x1111111111111111111111111111111111111111");
    let amount = 500000000000000000; // 0.5 tokens
    let deposit_amount = 1000000000000000000; // 1 token
    
    // Setup
    token.mint(payer, deposit_amount);
    token.set_caller(payer);
    token.approve(payments.address(), deposit_amount);
    
    payments.set_caller(payer);
    payments.deposit(deposit_amount);
    
    // Process payment
    payments.processPayment(payer, payee, amount);
    
    // Verify balances
    let payer_balance = payments.getBalance(payer);
    assert(payer_balance == deposit_amount - amount);
    
    let payee_balance = payments.getBalance(payee);
    assert(payee_balance == amount);
}

test "batch payment processing" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let payments = AugustPayments::new(token.address());
    
    let payer1 = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let payer2 = Address::from("0x1111111111111111111111111111111111111111");
    let payee1 = Address::from("0x2222222222222222222222222222222222222222");
    let payee2 = Address::from("0x3333333333333333333333333333333333333333");
    let deposit_amount = 1000000000000000000; // 1 token
    
    // Setup payers
    token.mint(payer1, deposit_amount);
    token.mint(payer2, deposit_amount);
    
    token.set_caller(payer1);
    token.approve(payments.address(), deposit_amount);
    payments.set_caller(payer1);
    payments.deposit(deposit_amount);
    
    token.set_caller(payer2);
    token.approve(payments.address(), deposit_amount);
    payments.set_caller(payer2);
    payments.deposit(deposit_amount);
    
    // Prepare batch data
    let payers = [payer1, payer2];
    let payees = [payee1, payee2];
    let amounts = [300000000000000000, 400000000000000000]; // 0.3 and 0.4 tokens
    
    // Process batch payments
    payments.set_caller(payments.owner());
    payments.batchProcessPayments(payers, payees, amounts);
    
    // Verify results
    let payer1_balance = payments.getBalance(payer1);
    assert(payer1_balance == deposit_amount - amounts[0]);
    
    let payer2_balance = payments.getBalance(payer2);
    assert(payer2_balance == deposit_amount - amounts[1]);
    
    let payee1_balance = payments.getBalance(payee1);
    assert(payee1_balance == amounts[0]);
    
    let payee2_balance = payments.getBalance(payee2);
    assert(payee2_balance == amounts[1]);
}

test "insufficient balance handling" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let payments = AugustPayments::new(token.address());
    
    let payer = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let payee = Address::from("0x1111111111111111111111111111111111111111");
    let small_deposit = 100000000000000000; // 0.1 tokens
    let large_amount = 500000000000000000; // 0.5 tokens
    
    // Setup with insufficient balance
    token.mint(payer, small_deposit);
    token.set_caller(payer);
    token.approve(payments.address(), small_deposit);
    
    payments.set_caller(payer);
    payments.deposit(small_deposit);
    
    // Try to process payment that exceeds balance
    let result = payments.try_processPayment(payer, payee, large_amount);
    assert(result.is_error());
    
    // Verify no changes occurred
    let payer_balance = payments.getBalance(payer);
    assert(payer_balance == small_deposit);
    
    let payee_balance = payments.getBalance(payee);
    assert(payee_balance == 0);
}

test "escrow timeout handling" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let payments = AugustPayments::new(token.address());
    
    let payer = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let payee = Address::from("0x1111111111111111111111111111111111111111");
    let amount = 1000000000000000000; // 1 token
    let escrow_id = "escrow-timeout-123";
    
    // Setup
    token.mint(payer, amount);
    token.set_caller(payer);
    token.approve(payments.address(), amount);
    
    payments.set_caller(payer);
    payments.deposit(amount);
    
    // Create escrow
    payments.createEscrow(escrow_id, payee, amount);
    
    // Simulate time passage (this would need to be implemented in the test framework)
    // For now, we'll test the timeout check function
    let is_expired = payments.isEscrowExpired(escrow_id);
    // Initially should not be expired
    assert(is_expired == false);
}

test "emergency withdrawal" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let payments = AugustPayments::new(token.address());
    
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let amount = 1000000000000000000; // 1 token
    
    // Setup
    token.mint(user, amount);
    token.set_caller(user);
    token.approve(payments.address(), amount);
    
    payments.set_caller(user);
    payments.deposit(amount);
    
    // Emergency withdrawal (as owner)
    payments.set_caller(payments.owner());
    payments.emergencyWithdraw(user);
    
    // Verify user balance is zero
    let user_balance = payments.getBalance(user);
    assert(user_balance == 0);
    
    // Verify tokens were returned to user
    let user_token_balance = token.balanceOf(user);
    assert(user_token_balance == amount);
}

test "contract authorization" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let payments = AugustPayments::new(token.address());
    
    let billing_contract = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let metering_contract = Address::from("0x1111111111111111111111111111111111111111");
    
    // Authorize contracts
    payments.set_caller(payments.owner());
    payments.authorizeContract(billing_contract);
    payments.authorizeContract(metering_contract);
    
    // Verify authorization
    let is_billing_authorized = payments.isAuthorizedContract(billing_contract);
    assert(is_billing_authorized == true);
    
    let is_metering_authorized = payments.isAuthorizedContract(metering_contract);
    assert(is_metering_authorized == true);
    
    // Test deauthorization
    payments.deauthorizeContract(billing_contract);
    let is_billing_still_authorized = payments.isAuthorizedContract(billing_contract);
    assert(is_billing_still_authorized == false);
}

test "minimum deposit enforcement" {
    let token = MockERC20::new("Test Token", "TEST", 18);
    let payments = AugustPayments::new(token.address());
    
    let user = Address::from("0xabcdefabcdefabcdefabcdefabcdefabcdefabcdef");
    let min_deposit = 100000000000000000; // 0.1 tokens
    let small_amount = 50000000000000000; // 0.05 tokens
    
    // Set minimum deposit
    payments.set_caller(payments.owner());
    payments.setMinimumDeposit(min_deposit);
    
    // Setup
    token.mint(user, small_amount);
    token.set_caller(user);
    token.approve(payments.address(), small_amount);
    
    // Try to deposit below minimum
    payments.set_caller(user);
    let result = payments.try_deposit(small_amount);
    assert(result.is_error());
    
    // Verify no deposit occurred
    let balance = payments.getBalance(user);
    assert(balance == 0);
}