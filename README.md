

## Overview

The AugustCredits system consists of three main smart contracts:

1. **AugustPayments** (`payments.aug`) - Handles deposits, withdrawals, and escrow functionality
2. **AugustCreditsBilling** (`billing.aug`) - Manages usage tracking, billing calculations, and payment processing
3. **AugustCreditsMetering** (`metering.aug`) - Handles API request monitoring, rate limiting, and usage metrics collection

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   AugustPayments │    │ AugustCredits   │    │ AugustCredits   │
│                 │    │    Billing      │    │    Metering     │
│ • Deposits      │◄──►│                 │◄──►│                 │
│ • Withdrawals   │    │ • Usage Tracking│    │ • Rate Limiting │
│ • Escrow        │    │ • Billing Calc  │    │ • Analytics     │
│ • Batch Payments│    │ • Payment Proc  │    │ • Request Logs  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                        ┌─────────────────┐
                        │   ERC20 Token   │
                        │   (USDC/USDT)   │
                        └─────────────────┘
```

## Contract Details

### AugustPayments

**Purpose**: Core payment infrastructure with escrow capabilities

**Key Features**:
- Secure deposit and withdrawal system
- Escrow creation and management with timeout protection
- Batch payment processing for efficiency
- Emergency withdrawal capabilities
- Contract authorization system

**Main Functions**:
- `deposit(amount)` - Deposit tokens to user balance
- `withdraw(amount)` - Withdraw tokens from user balance
- `createEscrow(id, payee, amount)` - Create escrow payment
- `releaseEscrow(id)` - Release escrow to payee
- `refundEscrow(id)` - Refund escrow to payer
- `processPayment(payer, payee, amount)` - Direct payment
- `batchProcessPayments(payers, payees, amounts)` - Batch payments

### AugustCreditsBilling

**Purpose**: Usage-based billing and payment processing

**Key Features**:
- User and API endpoint registration
- Dynamic pricing per endpoint
- Usage tracking and billing calculation
- Platform fee management
- Batch billing for multiple users
- Cost estimation and affordability checks

**Main Functions**:
- `registerUser(apiKey)` - Register new user
- `registerApiEndpoint(name, price)` - Register API endpoint
- `recordUsage(apiKey, endpoint, requests)` - Record API usage
- `estimateCost(endpoint, requests)` - Estimate usage cost
- `canAffordUsage(user, endpoint, requests)` - Check affordability
- `batchBilling(users, endpoints, requests)` - Process multiple bills

### AugustCreditsMetering

**Purpose**: API monitoring, rate limiting, and analytics

**Key Features**:
- Configurable rate limiting per endpoint
- Real-time request logging
- Usage analytics and reporting
- Daily usage tracking
- Batch analytics retrieval
- Log cleanup and management

**Main Functions**:
- `setRateLimit(endpoint, requests, period)` - Configure rate limits
- `checkRateLimit(user, endpoint)` - Check if request allowed
- `logRequest(user, endpoint, requestId, ...)` - Log API request
- `getEndpointStats(endpoint)` - Get endpoint analytics
- `getUserStats(user)` - Get user analytics
- `getDailyUsage(endpoint, day)` - Get daily usage stats

## Deployment

### Prerequisites

1. Install August CLI:
```bash
curl -sSL https://get.august.dev | bash
```

2. Configure your network settings in `august.toml`

### Deploy to Different Networks

#### Localhost (for development)
```bash
august deploy --network localhost
```

#### Goerli Testnet
```bash
august deploy --network goerli
```

#### Mainnet
```bash
august deploy --network mainnet
```

#### Polygon
```bash
august deploy --network polygon
```

### Using the Deployment Script

The `deploy.aug` script provides automated deployment and configuration:

```bash
# Deploy to localhost with mock tokens
NETWORK=localhost august run deploy.aug

# Deploy to testnet
NETWORK=goerli august run deploy.aug

# Deploy to mainnet
NETWORK=mainnet august run deploy.aug
```

## Testing

Run the comprehensive test suite:

```bash
# Run all tests
august test

# Run specific contract tests
august test tests/payments_test.aug
august test tests/billing_test.aug
august test tests/metering_test.aug
```

### Test Coverage

The test suite covers:
- ✅ Payment deposits and withdrawals
- ✅ Escrow creation, release, and refunds
- ✅ User and endpoint registration
- ✅ Usage recording and billing
- ✅ Rate limiting and enforcement
- ✅ Analytics and reporting
- ✅ Batch operations
- ✅ Error handling and edge cases
- ✅ Access control and authorization

## Configuration

### Environment Variables

```bash
# Network configuration
NETWORK=localhost|goerli|mainnet|polygon

# Contract addresses (set after deployment)
PAYMENTS_CONTRACT=0x...
BILLING_CONTRACT=0x...
METERING_CONTRACT=0x...

# Token addresses
USDC_MAINNET=0xA0b86a33E6441E6C7C5c8b7b8b8b8b8b8b8b8b8b
USDC_GOERLI=0x07865c6E87B9F70255377e024ace6630C1Eaa37F
USDC_POLYGON=0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174
```

### Default Settings

```toml
# Platform fee (basis points)
platform_fee = 250  # 2.5%

# Rate limiting
default_rate_limit = 1000  # requests per hour
default_rate_period = 3600  # 1 hour

# Payments
minimum_deposit = "100000000000000000"  # 0.1 tokens
escrow_timeout = 604800  # 7 days

# Gas settings
gas_limit = 3000000
gas_price = "20gwei"
```

## Security Considerations

### Access Control
- All contracts use OpenZeppelin's `Ownable` pattern
- Critical functions are protected by `onlyOwner` modifier
- Contract-to-contract calls are authorized via whitelist

### Rate Limiting
- Prevents API abuse and DoS attacks
- Configurable per endpoint
- Automatic reset based on time windows

### Escrow Protection
- Timeout mechanism prevents locked funds
- Only payer or contract owner can refund
- Atomic release operations

### Emergency Features
- Emergency withdrawal for user funds
- Contract pause functionality
- Upgrade mechanisms for future improvements

## Integration Guide

### Backend Integration

1. **API Gateway**: Integrate rate limiting with `metering.checkRateLimit()`
2. **Usage Tracking**: Log requests with `metering.logRequest()`
3. **Billing**: Process usage with `billing.recordUsage()`
4. **Analytics**: Retrieve stats with batch functions

### Example Integration

```javascript
// Backend: Check rate limit
const [allowed, remaining, resetTime] = await meteringContract.checkRateLimit(
  userAddress,
  "api-v1"
);

if (!allowed) {
  return res.status(429).json({ error: "Rate limit exceeded" });
}

// Backend: Log request
await meteringContract.logRequest(
  userAddress,
  "api-v1",
  requestId,
  responseTime,
  statusCode,
  ipHash
);
```

## Monitoring and Analytics

### Key Metrics
- Total requests per endpoint
- Unique users per endpoint
- Average response times
- Error rates
- Daily usage patterns
- Revenue tracking

### Analytics Queries

```javascript
// Get endpoint analytics
const stats = await meteringContract.getEndpointStats("api-v1");

// Get user usage
const userStats = await meteringContract.getUserStats(userAddress);

// Get daily usage
const today = Math.floor(Date.now() / 1000 / 86400) * 86400;
const dailyUsage = await meteringContract.getDailyUsage("api-v1", today);
```

## Troubleshooting

### Common Issues

1. **Insufficient Balance**: Ensure users have deposited enough tokens
2. **Rate Limit Exceeded**: Check rate limit settings and user usage
3. **Unauthorized Access**: Verify contract addresses are properly configured
4. **Gas Estimation Failed**: Check for contract pauses or network issues

### Debug Commands

```bash
# Check contract deployment
august verify --network <network>

# View contract state
august call <contract> <function> --network <network>

# Monitor events
august events <contract> --network <network>
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

### Development Setup

```bash
# Clone repository
git clone https://github.com/augustcredits/contracts.git
cd contracts

# Install dependencies
august install

# Run tests
august test

# Deploy locally
august deploy --network localhost
```

## License

MIT License - see [LICENSE](../LICENSE) file for details.

