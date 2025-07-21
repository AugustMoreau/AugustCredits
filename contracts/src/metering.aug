// AugustCredits Metering Contract
// Handles API request monitoring, rate limiting, and usage metrics collection

use std::math::SafeMath;
use std::access::Ownable;

contract AugustCreditsMetering is Ownable {
    // Events
    event RequestLogged(address indexed user, string apiEndpoint, uint256 timestamp, bytes32 requestId);
    event RateLimitUpdated(string apiEndpoint, uint256 requestsPerPeriod, uint256 periodDuration);
    event UsageThresholdReached(address indexed user, string apiEndpoint, uint256 currentUsage);
    event AnalyticsUpdated(string apiEndpoint, uint256 totalRequests, uint256 uniqueUsers);
    
    // Structs
    struct RateLimit {
        uint256 requestsPerPeriod;
        uint256 periodDuration; // in seconds
        bool isActive;
    }
    
    struct UserUsageWindow {
        uint256 requestCount;
        uint256 windowStart;
        uint256 lastRequestTime;
    }
    
    struct RequestLog {
        address user;
        string endpoint;
        uint256 timestamp;
        bytes32 requestId;
        uint256 responseTime;
        uint16 statusCode;
        bytes32 ipHash; // Hashed IP for privacy
    }
    
    struct EndpointAnalytics {
        uint256 totalRequests;
        uint256 uniqueUsers;
        uint256 averageResponseTime;
        uint256 errorRate; // basis points (e.g., 500 = 5%)
        uint256 lastUpdated;
        mapping<uint256 => uint256) dailyRequests; // day timestamp => request count
        mapping(address => bool) hasUsed; // track unique users
    }
    
    struct UserAnalytics {
        uint256 totalRequests;
        uint256 firstRequestTime;
        uint256 lastRequestTime;
        mapping<string => uint256) endpointUsage;
        mapping<uint256 => uint256) dailyUsage; // day timestamp => request count
    }
    
    // State variables
    mapping<string => RateLimit) public rateLimits;
    mapping<address => mapping(string => UserUsageWindow)) public userUsageWindows;
    mapping<string => EndpointAnalytics) public endpointAnalytics;
    mapping<address => UserAnalytics) public userAnalytics;
    
    RequestLog[] public requestLogs;
    mapping<bytes32 => uint256) public requestIdToLogIndex;
    
    uint256 public maxLogsRetention = 30 days;
    uint256 public defaultRateLimit = 1000; // requests per hour
    uint256 public defaultRatePeriod = 3600; // 1 hour
    
    address public billingContract;
    
    // Modifiers
    modifier onlyBillingContract() {
        require(msg.sender == billingContract, "Only billing contract can call");
        _;
    }
    
    modifier validEndpoint(string memory endpoint) {
        require(bytes(endpoint).length > 0, "Invalid endpoint");
        _;
    }
    
    // Constructor
    constructor(address _billingContract) {
        billingContract = _billingContract;
    }
    
    // Rate Limiting Functions
    
    pub fn setRateLimit(
        string memory endpoint,
        uint256 requestsPerPeriod,
        uint256 periodDuration
    ) external onlyOwner validEndpoint(endpoint) {
        require(requestsPerPeriod > 0, "Requests per period must be > 0");
        require(periodDuration > 0, "Period duration must be > 0");
        
        rateLimits[endpoint] = RateLimit({
            requestsPerPeriod: requestsPerPeriod,
            periodDuration: periodDuration,
            isActive: true
        });
        
        emit RateLimitUpdated(endpoint, requestsPerPeriod, periodDuration);
    }
    
    pub fn checkRateLimit(
        address user,
        string memory endpoint
    ) external view returns (bool allowed, uint256 remainingRequests, uint256 resetTime) {
        RateLimit memory limit = rateLimits[endpoint];
        
        if (!limit.isActive) {
            // Use default rate limit
            limit.requestsPerPeriod = defaultRateLimit;
            limit.periodDuration = defaultRatePeriod;
        }
        
        UserUsageWindow memory window = userUsageWindows[user][endpoint];
        uint256 currentTime = block.timestamp;
        
        // Check if we're in a new period
        if (currentTime >= window.windowStart + limit.periodDuration) {
            // New period, reset window
            allowed = true;
            remainingRequests = limit.requestsPerPeriod - 1;
            resetTime = currentTime + limit.periodDuration;
        } else {
            // Same period, check if under limit
            if (window.requestCount < limit.requestsPerPeriod) {
                allowed = true;
                remainingRequests = limit.requestsPerPeriod - window.requestCount - 1;
            } else {
                allowed = false;
                remainingRequests = 0;
            }
            resetTime = window.windowStart + limit.periodDuration;
        }
    }
    
    // Usage Tracking Functions
    
    pub fn logRequest(
        address user,
        string memory endpoint,
        bytes32 requestId,
        uint256 responseTime,
        uint16 statusCode,
        bytes32 ipHash
    ) external onlyBillingContract validEndpoint(endpoint) {
        uint256 currentTime = block.timestamp;
        
        // Update rate limiting window
        _updateUsageWindow(user, endpoint, currentTime);
        
        // Create request log
        RequestLog memory log = RequestLog({
            user: user,
            endpoint: endpoint,
            timestamp: currentTime,
            requestId: requestId,
            responseTime: responseTime,
            statusCode: statusCode,
            ipHash: ipHash
        });
        
        requestLogs.push(log);
        requestIdToLogIndex[requestId] = requestLogs.length - 1;
        
        // Update analytics
        _updateAnalytics(user, endpoint, responseTime, statusCode, currentTime);
        
        emit RequestLogged(user, endpoint, currentTime, requestId);
    }
    
    function _updateUsageWindow(
        address user,
        string memory endpoint,
        uint256 currentTime
    ) internal {
        RateLimit memory limit = rateLimits[endpoint];
        if (!limit.isActive) {
            limit.requestsPerPeriod = defaultRateLimit;
            limit.periodDuration = defaultRatePeriod;
        }
        
        UserUsageWindow storage window = userUsageWindows[user][endpoint];
        
        // Check if we need to reset the window
        if (currentTime >= window.windowStart + limit.periodDuration) {
            window.requestCount = 1;
            window.windowStart = currentTime;
        } else {
            window.requestCount += 1;
        }
        
        window.lastRequestTime = currentTime;
    }
    
    function _updateAnalytics(
        address user,
        string memory endpoint,
        uint256 responseTime,
        uint16 statusCode,
        uint256 timestamp
    ) internal {
        // Update endpoint analytics
        EndpointAnalytics storage endpointStats = endpointAnalytics[endpoint];
        endpointStats.totalRequests += 1;
        endpointStats.lastUpdated = timestamp;
        
        // Track unique users
        if (!endpointStats.hasUsed[user]) {
            endpointStats.hasUsed[user] = true;
            endpointStats.uniqueUsers += 1;
        }
        
        // Update average response time
        if (endpointStats.totalRequests == 1) {
            endpointStats.averageResponseTime = responseTime;
        } else {
            endpointStats.averageResponseTime = 
                (endpointStats.averageResponseTime * (endpointStats.totalRequests - 1) + responseTime) / 
                endpointStats.totalRequests;
        }
        
        // Update error rate
        if (statusCode >= 400) {
            uint256 errorCount = (endpointStats.errorRate * (endpointStats.totalRequests - 1)) / 10000 + 1;
            endpointStats.errorRate = (errorCount * 10000) / endpointStats.totalRequests;
        } else {
            uint256 errorCount = (endpointStats.errorRate * (endpointStats.totalRequests - 1)) / 10000;
            endpointStats.errorRate = (errorCount * 10000) / endpointStats.totalRequests;
        }
        
        // Update daily stats
        uint256 dayTimestamp = timestamp / 86400 * 86400; // Round to day
        endpointStats.dailyRequests[dayTimestamp] += 1;
        
        // Update user analytics
        UserAnalytics storage userStats = userAnalytics[user];
        userStats.totalRequests += 1;
        userStats.lastRequestTime = timestamp;
        userStats.endpointUsage[endpoint] += 1;
        userStats.dailyUsage[dayTimestamp] += 1;
        
        if (userStats.firstRequestTime == 0) {
            userStats.firstRequestTime = timestamp;
        }
        
        emit AnalyticsUpdated(endpoint, endpointStats.totalRequests, endpointStats.uniqueUsers);
    }
    
    // Analytics and Reporting Functions
    
    pub fn getEndpointStats(string memory endpoint) external view returns (
        uint256 totalRequests,
        uint256 uniqueUsers,
        uint256 averageResponseTime,
        uint256 errorRate
    ) {
        EndpointAnalytics memory stats = endpointAnalytics[endpoint];
        return (
            stats.totalRequests,
            stats.uniqueUsers,
            stats.averageResponseTime,
            stats.errorRate
        );
    }
    
    pub fn getUserStats(address user) external view returns (
        uint256 totalRequests,
        uint256 firstRequestTime,
        uint256 lastRequestTime
    ) {
        UserAnalytics memory stats = userAnalytics[user];
        return (
            stats.totalRequests,
            stats.firstRequestTime,
            stats.lastRequestTime
        );
    }
    
    pub fn getDailyUsage(
        string memory endpoint,
        uint256 dayTimestamp
    ) external view returns (uint256) {
        return endpointAnalytics[endpoint].dailyRequests[dayTimestamp];
    }
    
    pub fn getUserDailyUsage(
        address user,
        uint256 dayTimestamp
    ) external view returns (uint256) {
        return userAnalytics[user].dailyUsage[dayTimestamp];
    }
    
    pub fn getUserEndpointUsage(
        address user,
        string memory endpoint
    ) external view returns (uint256) {
        return userAnalytics[user].endpointUsage[endpoint];
    }
    
    // Batch analytics for monitoring
    pub fn getEndpointAnalyticsBatch(
        string[] memory endpoints
    ) external view returns (
        uint256[] memory totalRequests,
        uint256[] memory uniqueUsers,
        uint256[] memory averageResponseTimes,
        uint256[] memory errorRates
    ) {
        uint256 length = endpoints.length;
        totalRequests = new uint256[](length);
        uniqueUsers = new uint256[](length);
        averageResponseTimes = new uint256[](length);
        errorRates = new uint256[](length);
        
        for (uint256 i = 0; i < length; i++) {
            EndpointAnalytics memory stats = endpointAnalytics[endpoints[i]];
            totalRequests[i] = stats.totalRequests;
            uniqueUsers[i] = stats.uniqueUsers;
            averageResponseTimes[i] = stats.averageResponseTime;
            errorRates[i] = stats.errorRate;
        }
    }
    
    // Log Management
    
    pub fn getRequestLog(bytes32 requestId) external view returns (
        address user,
        string memory endpoint,
        uint256 timestamp,
        uint256 responseTime,
        uint16 statusCode
    ) {
        uint256 index = requestIdToLogIndex[requestId];
        require(index < requestLogs.length, "Request log not found");
        
        RequestLog memory log = requestLogs[index];
        return (
            log.user,
            log.endpoint,
            log.timestamp,
            log.responseTime,
            log.statusCode
        );
    }
    
    pub fn getRecentLogs(
        uint256 count,
        uint256 offset
    ) external view returns (RequestLog[] memory) {
        require(offset < requestLogs.length, "Offset out of bounds");
        
        uint256 end = offset + count;
        if (end > requestLogs.length) {
            end = requestLogs.length;
        }
        
        RequestLog[] memory logs = new RequestLog[](end - offset);
        for (uint256 i = offset; i < end; i++) {
            logs[i - offset] = requestLogs[requestLogs.length - 1 - i]; // Most recent first
        }
        
        return logs;
    }
    
    // Admin Functions
    
    pub fn setBillingContract(address newBillingContract) external onlyOwner {
        require(newBillingContract != address(0), "Invalid billing contract");
        billingContract = newBillingContract;
    }
    
    pub fn updateDefaultRateLimit(
        uint256 requestsPerPeriod,
        uint256 periodDuration
    ) external onlyOwner {
        require(requestsPerPeriod > 0, "Requests per period must be > 0");
        require(periodDuration > 0, "Period duration must be > 0");
        
        defaultRateLimit = requestsPerPeriod;
        defaultRatePeriod = periodDuration;
    }
    
    pub fn cleanupOldLogs(uint256 beforeTimestamp) external onlyOwner {
        // Remove logs older than specified timestamp
        // This is a simplified version - in practice, you'd want a more efficient cleanup
        uint256 i = 0;
        while (i < requestLogs.length) {
            if (requestLogs[i].timestamp < beforeTimestamp) {
                // Remove this log (simplified - move last element to this position)
                requestLogs[i] = requestLogs[requestLogs.length - 1];
                requestLogs.pop();
            } else {
                i++;
            }
        }
    }
    
    pub fn pauseRateLimit(string memory endpoint) external onlyOwner {
        rateLimits[endpoint].isActive = false;
    }
    
    pub fn unpauseRateLimit(string memory endpoint) external onlyOwner {
        rateLimits[endpoint].isActive = true;
    }
}