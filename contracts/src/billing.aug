/// AugustCredits Billing Contract
/// 
/// Core billing system that tracks API usage, calculates costs, and manages
/// user accounts and endpoint monetization. Integrates with payment processing
/// and metering contracts for complete billing automation.

contract AugustBilling {
    /// Core state mappings for users, endpoints, and usage tracking
    mapping(address => UserAccount) public userAccounts;
    mapping(bytes32 => ApiEndpoint) public apiEndpoints;
    mapping(bytes32 => UsageRecord) public usageRecords;
    mapping(address => bytes32[]) public userEndpoints;
    mapping(bytes32 => bytes32[]) public endpointUsageHistory;
    
    address public owner;
    address public paymentsContract;
    address public meteringContract;
    
    uint256 public platformFeePercent = 250; // 2.5% platform fee (basis points)
    uint256 public minimumCharge = 0.0001 ether; // Minimum charge per request
    uint256 public maxMonthlyLimit = 1000000; // Default monthly limit
    
    /// Events emitted for important billing operations
    event UserRegistered(address indexed user, string name, uint256 timestamp);
    event EndpointRegistered(bytes32 indexed endpointId, address indexed owner, string url, uint256 pricePerRequest);
    event EndpointUpdated(bytes32 indexed endpointId, uint256 newPrice, bool isActive);
    event UsageRecorded(bytes32 indexed recordId, address indexed user, bytes32 indexed endpointId, uint256 requests, uint256 cost);
    event BillGenerated(address indexed user, uint256 totalAmount, uint256 platformFee, uint256 timestamp);
    event PaymentProcessed(address indexed user, bytes32 indexed endpointId, uint256 amount, uint256 timestamp);
    event MonthlyLimitUpdated(address indexed user, uint256 newLimit);
    event PlatformFeeUpdated(uint256 newFeePercent);
    
    /// Data structures for billing entities
    struct UserAccount {
        string name;
        string email;
        uint256 monthlyLimit;
        uint256 currentMonthUsage;
        uint256 totalSpent;
        uint256 lastBillingDate;
        uint256 registeredAt;
        bool isActive;
        uint256 outstandingBalance;
    }
    
    struct ApiEndpoint {
        address owner;
        string url;
        string description;
        uint256 pricePerRequest;
        uint256 totalRequests;
        uint256 totalRevenue;
        uint256 createdAt;
        uint256 updatedAt;
        bool isActive;
        bool requiresAuth;
        mapping(address => bool) authorizedUsers;
    }
    
    struct UsageRecord {
        address user;
        bytes32 endpointId;
        uint256 requestCount;
        uint256 totalCost;
        uint256 timestamp;
        uint256 responseTime;
        uint256 dataTransferred;
        bool isPaid;
        string metadata;
    }
    
    struct BillingPeriod {
        uint256 startDate;
        uint256 endDate;
        uint256 totalRequests;
        uint256 totalCost;
        uint256 platformFee;
        bool isPaid;
    }
    
    /// Access control and validation modifiers
    modifier onlyOwner() {
        require(msg.sender == owner, "Only owner can call this function");
        _;
    }
    
    modifier onlyPaymentsContract() {
        require(msg.sender == paymentsContract, "Only payments contract can call this");
        _;
    }
    
    modifier onlyMeteringContract() {
        require(msg.sender == meteringContract, "Only metering contract can call this");
        _;
    }
    
    modifier validEndpoint(bytes32 endpointId) {
        require(apiEndpoints[endpointId].owner != address(0), "Endpoint does not exist");
        require(apiEndpoints[endpointId].isActive, "Endpoint is not active");
        _;
    }
    
    modifier registeredUser(address user) {
        require(userAccounts[user].registeredAt > 0, "User not registered");
        require(userAccounts[user].isActive, "User account is inactive");
        _;
    }
    
    /// Initializes the billing contract with the deployer as owner
    constructor() {
        owner = msg.sender;
    }
    
    /// User account registration and management
    /// Registers a new user account with basic profile information
    function registerUser(
        string memory name,
        string memory email
    ) external {
        require(userAccounts[msg.sender].registeredAt == 0, "User already registered");
        require(bytes(name).length > 0, "Name cannot be empty");
        
        userAccounts[msg.sender] = UserAccount({
            name: name,
            email: email,
            monthlyLimit: maxMonthlyLimit,
            currentMonthUsage: 0,
            totalSpent: 0,
            lastBillingDate: block.timestamp,
            registeredAt: block.timestamp,
            isActive: true,
            outstandingBalance: 0
        });
        
        emit UserRegistered(msg.sender, name, block.timestamp);
    }
    
    /// Updates user profile information for existing accounts
    function updateUserProfile(
        string memory name,
        string memory email
    ) external registeredUser(msg.sender) {
        UserAccount storage account = userAccounts[msg.sender];
        account.name = name;
        account.email = email;
    }
    
    /// Allows users to adjust their monthly usage limits
    function setMonthlyLimit(uint256 newLimit) external registeredUser(msg.sender) {
        require(newLimit > 0, "Monthly limit must be greater than 0");
        userAccounts[msg.sender].monthlyLimit = newLimit;
        emit MonthlyLimitUpdated(msg.sender, newLimit);
    }
    
    /// API endpoint registration and configuration
    /// Registers a new API endpoint for monetization
    function registerEndpoint(
        bytes32 endpointId,
        string memory url,
        string memory description,
        uint256 pricePerRequest,
        bool requiresAuth
    ) external registeredUser(msg.sender) {
        require(apiEndpoints[endpointId].owner == address(0), "Endpoint already exists");
        require(bytes(url).length > 0, "URL cannot be empty");
        require(pricePerRequest >= minimumCharge, "Price below minimum charge");
        
        ApiEndpoint storage endpoint = apiEndpoints[endpointId];
        endpoint.owner = msg.sender;
        endpoint.url = url;
        endpoint.description = description;
        endpoint.pricePerRequest = pricePerRequest;
        endpoint.totalRequests = 0;
        endpoint.totalRevenue = 0;
        endpoint.createdAt = block.timestamp;
        endpoint.updatedAt = block.timestamp;
        endpoint.isActive = true;
        endpoint.requiresAuth = requiresAuth;
        
        userEndpoints[msg.sender].push(endpointId);
        
        emit EndpointRegistered(endpointId, msg.sender, url, pricePerRequest);
    }
    
    /// Updates pricing and availability for existing endpoints
    function updateEndpoint(
        bytes32 endpointId,
        uint256 newPrice,
        bool isActive
    ) external {
        ApiEndpoint storage endpoint = apiEndpoints[endpointId];
        require(endpoint.owner == msg.sender, "Only endpoint owner can update");
        require(newPrice >= minimumCharge, "Price below minimum charge");
        
        endpoint.pricePerRequest = newPrice;
        endpoint.isActive = isActive;
        endpoint.updatedAt = block.timestamp;
        
        emit EndpointUpdated(endpointId, newPrice, isActive);
    }
    
    /// Manages user authorization for private endpoints
    function authorizeUser(bytes32 endpointId, address user, bool authorized) external {
        ApiEndpoint storage endpoint = apiEndpoints[endpointId];
        require(endpoint.owner == msg.sender, "Only endpoint owner can authorize users");
        require(endpoint.requiresAuth, "Endpoint does not require authorization");
        
        endpoint.authorizedUsers[user] = authorized;
    }
    
    /// Core usage recording and billing calculation logic
    /// Records API usage and calculates billing charges
    function recordUsage(
        bytes32 recordId,
        address user,
        bytes32 endpointId,
        uint256 requestCount,
        uint256 responseTime,
        uint256 dataTransferred,
        string memory metadata
    ) external onlyMeteringContract validEndpoint(endpointId) registeredUser(user) {
        require(usageRecords[recordId].user == address(0), "Usage record already exists");
        
        ApiEndpoint storage endpoint = apiEndpoints[endpointId];
        UserAccount storage account = userAccounts[user];
        
        // Check authorization if required
        if (endpoint.requiresAuth) {
            require(endpoint.authorizedUsers[user], "User not authorized for this endpoint");
        }
        
        // Check monthly limit
        require(
            account.currentMonthUsage + requestCount <= account.monthlyLimit,
            "Monthly usage limit exceeded"
        );
        
        // Calculate cost
        uint256 totalCost = requestCount * endpoint.pricePerRequest;
        
        // Create usage record
        usageRecords[recordId] = UsageRecord({
            user: user,
            endpointId: endpointId,
            requestCount: requestCount,
            totalCost: totalCost,
            timestamp: block.timestamp,
            responseTime: responseTime,
            dataTransferred: dataTransferred,
            isPaid: false,
            metadata: metadata
        });
        
        // Update endpoint stats
        endpoint.totalRequests += requestCount;
        endpoint.totalRevenue += totalCost;
        
        // Update user stats
        account.currentMonthUsage += requestCount;
        account.outstandingBalance += totalCost;
        
        // Add to history
        endpointUsageHistory[endpointId].push(recordId);
        
        emit UsageRecorded(recordId, user, endpointId, requestCount, totalCost);
    }
    
    function processPayment(
        bytes32 recordId
    ) external onlyPaymentsContract {
        UsageRecord storage record = usageRecords[recordId];
        require(record.user != address(0), "Usage record does not exist");
        require(!record.isPaid, "Usage already paid");
        
        ApiEndpoint storage endpoint = apiEndpoints[record.endpointId];
        UserAccount storage account = userAccounts[record.user];
        
        // Calculate platform fee
        uint256 platformFee = (record.totalCost * platformFeePercent) / 10000;
        uint256 endpointOwnerAmount = record.totalCost - platformFee;
        
        // Mark as paid
        record.isPaid = true;
        
        // Update user account
        account.totalSpent += record.totalCost;
        account.outstandingBalance -= record.totalCost;
        
        emit PaymentProcessed(record.user, record.endpointId, record.totalCost, block.timestamp);
    }
    
    function generateBill(address user) external view registeredUser(user) returns (
        uint256 totalAmount,
        uint256 platformFee,
        uint256 requestCount,
        bytes32[] memory unpaidRecords
    ) {
        UserAccount memory account = userAccounts[user];
        totalAmount = account.outstandingBalance;
        platformFee = (totalAmount * platformFeePercent) / 10000;
        
        // Count unpaid records and requests
        bytes32[] memory userEndpointsList = userEndpoints[user];
        uint256 totalUnpaidRecords = 0;
        requestCount = 0;
        
        // First pass: count unpaid records
        for (uint256 i = 0; i < userEndpointsList.length; i++) {
            bytes32[] memory endpointHistory = endpointUsageHistory[userEndpointsList[i]];
            for (uint256 j = 0; j < endpointHistory.length; j++) {
                UsageRecord memory record = usageRecords[endpointHistory[j]];
                if (record.user == user && !record.isPaid) {
                    totalUnpaidRecords++;
                    requestCount += record.requestCount;
                }
            }
        }
        
        // Second pass: collect unpaid record IDs
        unpaidRecords = new bytes32[](totalUnpaidRecords);
        uint256 index = 0;
        
        for (uint256 i = 0; i < userEndpointsList.length; i++) {
            bytes32[] memory endpointHistory = endpointUsageHistory[userEndpointsList[i]];
            for (uint256 j = 0; j < endpointHistory.length; j++) {
                UsageRecord memory record = usageRecords[endpointHistory[j]];
                if (record.user == user && !record.isPaid) {
                    unpaidRecords[index] = endpointHistory[j];
                    index++;
                }
            }
        }
    }
    
    function resetMonthlyUsage(address user) external onlyOwner registeredUser(user) {
        userAccounts[user].currentMonthUsage = 0;
        userAccounts[user].lastBillingDate = block.timestamp;
    }
    
    function batchResetMonthlyUsage(address[] memory users) external onlyOwner {
        for (uint256 i = 0; i < users.length; i++) {
            if (userAccounts[users[i]].registeredAt > 0) {
                userAccounts[users[i]].currentMonthUsage = 0;
                userAccounts[users[i]].lastBillingDate = block.timestamp;
            }
        }
    }
    
    // Administrative functions
    function setPaymentsContract(address _paymentsContract) external onlyOwner {
        require(_paymentsContract != address(0), "Invalid payments contract");
        paymentsContract = _paymentsContract;
    }
    
    function setMeteringContract(address _meteringContract) external onlyOwner {
        require(_meteringContract != address(0), "Invalid metering contract");
        meteringContract = _meteringContract;
    }
    
    function setPlatformFee(uint256 newFeePercent) external onlyOwner {
        require(newFeePercent <= 1000, "Platform fee cannot exceed 10%");
        platformFeePercent = newFeePercent;
        emit PlatformFeeUpdated(newFeePercent);
    }
    
    function setMinimumCharge(uint256 newMinimumCharge) external onlyOwner {
        require(newMinimumCharge > 0, "Minimum charge must be greater than 0");
        minimumCharge = newMinimumCharge;
    }
    
    function setMaxMonthlyLimit(uint256 newMaxLimit) external onlyOwner {
        require(newMaxLimit > 0, "Max monthly limit must be greater than 0");
        maxMonthlyLimit = newMaxLimit;
    }
    
    function deactivateUser(address user) external onlyOwner {
        require(userAccounts[user].registeredAt > 0, "User not registered");
        userAccounts[user].isActive = false;
    }
    
    function reactivateUser(address user) external onlyOwner {
        require(userAccounts[user].registeredAt > 0, "User not registered");
        userAccounts[user].isActive = true;
    }
    
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Invalid new owner");
        require(newOwner != owner, "Same owner");
        owner = newOwner;
    }
    
    // View functions
    function getUserAccount(address user) external view returns (
        string memory name,
        string memory email,
        uint256 monthlyLimit,
        uint256 currentMonthUsage,
        uint256 totalSpent,
        uint256 outstandingBalance,
        bool isActive
    ) {
        UserAccount memory account = userAccounts[user];
        return (
            account.name,
            account.email,
            account.monthlyLimit,
            account.currentMonthUsage,
            account.totalSpent,
            account.outstandingBalance,
            account.isActive
        );
    }
    
    function getEndpointDetails(bytes32 endpointId) external view returns (
        address owner,
        string memory url,
        string memory description,
        uint256 pricePerRequest,
        uint256 totalRequests,
        uint256 totalRevenue,
        bool isActive,
        bool requiresAuth
    ) {
        ApiEndpoint storage endpoint = apiEndpoints[endpointId];
        return (
            endpoint.owner,
            endpoint.url,
            endpoint.description,
            endpoint.pricePerRequest,
            endpoint.totalRequests,
            endpoint.totalRevenue,
            endpoint.isActive,
            endpoint.requiresAuth
        );
    }
    
    function getUsageRecord(bytes32 recordId) external view returns (
        address user,
        bytes32 endpointId,
        uint256 requestCount,
        uint256 totalCost,
        uint256 timestamp,
        bool isPaid
    ) {
        UsageRecord memory record = usageRecords[recordId];
        return (
            record.user,
            record.endpointId,
            record.requestCount,
            record.totalCost,
            record.timestamp,
            record.isPaid
        );
    }
    
    function getUserEndpoints(address user) external view returns (bytes32[] memory) {
        return userEndpoints[user];
    }
    
    function getEndpointUsageHistory(bytes32 endpointId) external view returns (bytes32[] memory) {
        return endpointUsageHistory[endpointId];
    }
    
    function isUserAuthorized(bytes32 endpointId, address user) external view returns (bool) {
        ApiEndpoint storage endpoint = apiEndpoints[endpointId];
        if (!endpoint.requiresAuth) {
            return true;
        }
        return endpoint.authorizedUsers[user];
    }
    
    function getPlatformStats() external view returns (
        uint256 totalEndpoints,
        uint256 totalUsers,
        uint256 totalRevenue,
        uint256 platformFeeCollected
    ) {
        // This would require additional tracking variables
        // Implementation depends on specific requirements
        return (0, 0, 0, 0);
    }
}