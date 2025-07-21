// AugustCredits Deployment Script
// Deploys and configures the AugustCredits smart contract system

use std::deploy::Deployer;
use std::token::ERC20;
use std::address::Address;
use std::console::log;

// Configuration constants
const PLATFORM_FEE_BASIS_POINTS: u256 = 250; // 2.5%
const DEFAULT_RATE_LIMIT: u256 = 1000; // requests per hour
const DEFAULT_RATE_PERIOD: u256 = 3600; // 1 hour in seconds
const MINIMUM_DEPOSIT: u256 = 100000000000000000; // 0.1 tokens
const ESCROW_TIMEOUT: u256 = 604800; // 7 days in seconds

struct DeploymentConfig {
    token_address: Address,
    treasury_address: Address,
    owner_address: Address,
    network: String,
}

struct DeployedContracts {
    payments: Address,
    billing: Address,
    metering: Address,
}

pub fn deploy_contracts(config: DeploymentConfig) -> DeployedContracts {
    log("Starting AugustCredits deployment...");
    log(format!("Network: {}", config.network));
    log(format!("Token: {}", config.token_address));
    log(format!("Treasury: {}", config.treasury_address));
    log(format!("Owner: {}", config.owner_address));
    
    // Deploy Payments contract first
    log("Deploying AugustPayments contract...");
    let payments = AugustPayments::deploy(config.token_address);
    log(format!("AugustPayments deployed at: {}", payments.address()));
    
    // Deploy Billing contract
    log("Deploying AugustCreditsBilling contract...");
    let billing = AugustCreditsBilling::deploy(
        config.token_address,
        config.treasury_address
    );
    log(format!("AugustCreditsBilling deployed at: {}", billing.address()));
    
    // Deploy Metering contract
    log("Deploying AugustCreditsMetering contract...");
    let metering = AugustCreditsMetering::deploy(billing.address());
    log(format!("AugustCreditsMetering deployed at: {}", metering.address()));
    
    // Configure contracts
    configure_contracts(&payments, &billing, &metering, &config);
    
    DeployedContracts {
        payments: payments.address(),
        billing: billing.address(),
        metering: metering.address(),
    }
}

fn configure_contracts(
    payments: &AugustPayments,
    billing: &AugustCreditsBilling,
    metering: &AugustCreditsMetering,
    config: &DeploymentConfig
) {
    log("Configuring contracts...");
    
    // Configure Payments contract
    log("Configuring AugustPayments...");
    payments.authorizeContract(billing.address());
    payments.authorizeContract(metering.address());
    payments.setMinimumDeposit(MINIMUM_DEPOSIT);
    payments.setEscrowTimeout(ESCROW_TIMEOUT);
    payments.setBillingContract(billing.address());
    payments.setMeteringContract(metering.address());
    
    // Configure Billing contract
    log("Configuring AugustCreditsBilling...");
    billing.setPaymentsContract(payments.address());
    billing.setMeteringContract(metering.address());
    billing.setPlatformFee(PLATFORM_FEE_BASIS_POINTS);
    
    // Configure Metering contract
    log("Configuring AugustCreditsMetering...");
    metering.setBillingContract(billing.address());
    metering.updateDefaultRateLimit(DEFAULT_RATE_LIMIT, DEFAULT_RATE_PERIOD);
    
    // Transfer ownership if different from deployer
    if config.owner_address != Deployer::caller() {
        log(format!("Transferring ownership to: {}", config.owner_address));
        payments.transferOwnership(config.owner_address);
        billing.transferOwnership(config.owner_address);
        metering.transferOwnership(config.owner_address);
    }
    
    log("Contract configuration completed!");
}

pub fn deploy_mainnet() -> DeployedContracts {
    let config = DeploymentConfig {
        token_address: Address::from("0xA0b86a33E6441E6C7C5c8b7b8b8b8b8b8b8b8b8b"), // USDC on mainnet
        treasury_address: Address::from("0x742d35Cc6634C0532925a3b8D4C9db7C4c4c4c4c"),
        owner_address: Address::from("0x123456789012345678901234567890123456789"),
        network: "mainnet".to_string(),
    };
    
    deploy_contracts(config)
}

pub fn deploy_goerli() -> DeployedContracts {
    let config = DeploymentConfig {
        token_address: Address::from("0x07865c6E87B9F70255377e024ace6630C1Eaa37F"), // USDC on Goerli
        treasury_address: Address::from("0x742d35Cc6634C0532925a3b8D4C9db7C4c4c4c4c"),
        owner_address: Address::from("0x123456789012345678901234567890123456789"),
        network: "goerli".to_string(),
    };
    
    deploy_contracts(config)
}

pub fn deploy_polygon() -> DeployedContracts {
    let config = DeploymentConfig {
        token_address: Address::from("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174"), // USDC on Polygon
        treasury_address: Address::from("0x742d35Cc6634C0532925a3b8D4C9db7C4c4c4c4c"),
        owner_address: Address::from("0x123456789012345678901234567890123456789"),
        network: "polygon".to_string(),
    };
    
    deploy_contracts(config)
}

pub fn deploy_localhost() -> DeployedContracts {
    // Deploy a mock ERC20 token for local testing
    log("Deploying mock USDC token for localhost...");
    let mock_token = MockERC20::deploy("Mock USDC", "USDC", 6);
    log(format!("Mock USDC deployed at: {}", mock_token.address()));
    
    let config = DeploymentConfig {
        token_address: mock_token.address(),
        treasury_address: Address::from("0x742d35Cc6634C0532925a3b8D4C9db7C4c4c4c4c"),
        owner_address: Deployer::caller(),
        network: "localhost".to_string(),
    };
    
    let contracts = deploy_contracts(config);
    
    // Mint some tokens for testing
    log("Minting test tokens...");
    let test_amount = 1000000000000; // 1M USDC (6 decimals)
    mock_token.mint(Deployer::caller(), test_amount);
    mock_token.mint(config.treasury_address, test_amount);
    
    contracts
}

// Verification functions
pub fn verify_deployment(contracts: &DeployedContracts) -> bool {
    log("Verifying deployment...");
    
    let payments = AugustPayments::at(contracts.payments);
    let billing = AugustCreditsBilling::at(contracts.billing);
    let metering = AugustCreditsMetering::at(contracts.metering);
    
    // Verify contract addresses are set correctly
    let billing_in_payments = payments.billingContract();
    let metering_in_payments = payments.meteringContract();
    let payments_in_billing = billing.paymentsContract();
    let metering_in_billing = billing.meteringContract();
    let billing_in_metering = metering.billingContract();
    
    let verification_passed = 
        billing_in_payments == contracts.billing &&
        metering_in_payments == contracts.metering &&
        payments_in_billing == contracts.payments &&
        metering_in_billing == contracts.metering &&
        billing_in_metering == contracts.billing;
    
    if verification_passed {
        log("✅ Deployment verification passed!");
        log("Contract addresses:");
        log(format!("  Payments: {}", contracts.payments));
        log(format!("  Billing: {}", contracts.billing));
        log(format!("  Metering: {}", contracts.metering));
    } else {
        log("❌ Deployment verification failed!");
        log("Contract address mismatches detected.");
    }
    
    verification_passed
}

// Upgrade functions for future use
pub fn upgrade_payments(new_implementation: Address, contracts: &DeployedContracts) {
    log("Upgrading AugustPayments contract...");
    let payments = AugustPayments::at(contracts.payments);
    payments.upgrade(new_implementation);
    log("AugustPayments upgrade completed!");
}

pub fn upgrade_billing(new_implementation: Address, contracts: &DeployedContracts) {
    log("Upgrading AugustCreditsBilling contract...");
    let billing = AugustCreditsBilling::at(contracts.billing);
    billing.upgrade(new_implementation);
    log("AugustCreditsBilling upgrade completed!");
}

pub fn upgrade_metering(new_implementation: Address, contracts: &DeployedContracts) {
    log("Upgrading AugustCreditsMetering contract...");
    let metering = AugustCreditsMetering::at(contracts.metering);
    metering.upgrade(new_implementation);
    log("AugustCreditsMetering upgrade completed!");
}

// Emergency functions
pub fn emergency_pause_all(contracts: &DeployedContracts) {
    log("🚨 Emergency pause activated for all contracts!");
    
    let payments = AugustPayments::at(contracts.payments);
    let billing = AugustCreditsBilling::at(contracts.billing);
    
    payments.pause();
    billing.pause();
    
    log("All contracts paused successfully.");
}

pub fn emergency_unpause_all(contracts: &DeployedContracts) {
    log("Unpausing all contracts...");
    
    let payments = AugustPayments::at(contracts.payments);
    let billing = AugustCreditsBilling::at(contracts.billing);
    
    payments.unpause();
    billing.unpause();
    
    log("All contracts unpaused successfully.");
}

// Main deployment entry point
pub fn main() {
    let network = std::env::var("NETWORK").unwrap_or("localhost".to_string());
    
    let contracts = match network.as_str() {
        "mainnet" => deploy_mainnet(),
        "goerli" => deploy_goerli(),
        "polygon" => deploy_polygon(),
        "localhost" | _ => deploy_localhost(),
    };
    
    if verify_deployment(&contracts) {
        log("🎉 AugustCredits deployment completed successfully!");
        
        // Save deployment info to file
        save_deployment_info(&contracts, &network);
    } else {
        log("💥 Deployment failed verification!");
        std::process::exit(1);
    }
}

fn save_deployment_info(contracts: &DeployedContracts, network: &str) {
    let deployment_info = format!(
        r#"{{
  "network": "{}",
  "timestamp": {},
  "contracts": {{
    "payments": "{}",
    "billing": "{}",
    "metering": "{}"
  }}
}}"#,
        network,
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
        contracts.payments,
        contracts.billing,
        contracts.metering
    );
    
    let filename = format!("deployment-{}.json", network);
    std::fs::write(&filename, deployment_info).expect("Failed to write deployment info");
    log(format!("Deployment info saved to {}", filename));
}