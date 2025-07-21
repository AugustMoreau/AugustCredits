// AugustCredits Payment Contract
// Handles deposits, withdrawals, escrow, and payment processing

contract AugustPayments {
    // State variables
    mapping(address => uint256) public balances;
    mapping(address => uint256) public escrowBalances;
    mapping(bytes32 => EscrowDeposit) public escrowDeposits;
    mapping(address => bool) public authorizedContracts;
    
    address public owner;
    address public billingContract;
    address public meteringContract;
    
    uint256 public totalDeposits;
    uint256 public totalEscrow;
    uint256 public minimumDeposit = 0.001 ether;
    uint256 public escrowTimeout = 7 days;
    
    // Events
    event Deposit(address indexed user, uint256 amount, uint256 timestamp);
    event Withdrawal(address indexed user, uint256 amount, uint256 timestamp);
    event EscrowCreated(bytes32 indexed escrowId, address indexed user, uint256 amount, uint256 timeout);
    event EscrowReleased(bytes32 indexed escrowId, address indexed user, uint256 amount);
    event EscrowRefunded(bytes32 indexed escrowId, address indexed user, uint256 amount);
    event PaymentProcessed(address indexed from, address indexed to, uint256 amount, bytes32 indexed transactionId);
    event ContractAuthorized(address indexed contractAddress, bool authorized);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    
    // Structs
    struct EscrowDeposit {
        address user;
        uint256 amount;
        uint256 createdAt;
        uint256 timeout;
        bool released;
        bool refunded;
        string purpose;
    }
    
    // Modifiers
    modifier onlyOwner() {
        require(msg.sender == owner, "Only owner can call this function");
        _;
    }
    
    modifier onlyAuthorized() {
        require(authorizedContracts[msg.sender] || msg.sender == owner, "Not authorized");
        _;
    }
    
    modifier validAmount(uint256 amount) {
        require(amount > 0, "Amount must be greater than 0");
        require(amount >= minimumDeposit, "Amount below minimum deposit");
        _;
    }
    
    modifier sufficientBalance(address user, uint256 amount) {
        require(balances[user] >= amount, "Insufficient balance");
        _;
    }
    
    // Constructor
    constructor() {
        owner = msg.sender;
        authorizedContracts[msg.sender] = true;
    }
    
    // Deposit functions
    function deposit() external payable validAmount(msg.value) {
        balances[msg.sender] += msg.value;
        totalDeposits += msg.value;
        
        emit Deposit(msg.sender, msg.value, block.timestamp);
    }
    
    function depositFor(address user) external payable validAmount(msg.value) {
        require(user != address(0), "Invalid user address");
        
        balances[user] += msg.value;
        totalDeposits += msg.value;
        
        emit Deposit(user, msg.value, block.timestamp);
    }
    
    // Withdrawal functions
    function withdraw(uint256 amount) external sufficientBalance(msg.sender, amount) {
        balances[msg.sender] -= amount;
        totalDeposits -= amount;
        
        (bool success, ) = msg.sender.call{value: amount}("");
        require(success, "Withdrawal failed");
        
        emit Withdrawal(msg.sender, amount, block.timestamp);
    }
    
    function withdrawAll() external {
        uint256 amount = balances[msg.sender];
        require(amount > 0, "No balance to withdraw");
        
        balances[msg.sender] = 0;
        totalDeposits -= amount;
        
        (bool success, ) = msg.sender.call{value: amount}("");
        require(success, "Withdrawal failed");
        
        emit Withdrawal(msg.sender, amount, block.timestamp);
    }
    
    // Escrow functions
    function createEscrow(
        bytes32 escrowId,
        uint256 amount,
        string memory purpose
    ) external sufficientBalance(msg.sender, amount) {
        require(escrowDeposits[escrowId].user == address(0), "Escrow already exists");
        
        balances[msg.sender] -= amount;
        escrowBalances[msg.sender] += amount;
        totalEscrow += amount;
        
        escrowDeposits[escrowId] = EscrowDeposit({
            user: msg.sender,
            amount: amount,
            createdAt: block.timestamp,
            timeout: block.timestamp + escrowTimeout,
            released: false,
            refunded: false,
            purpose: purpose
        });
        
        emit EscrowCreated(escrowId, msg.sender, amount, block.timestamp + escrowTimeout);
    }
    
    function releaseEscrow(bytes32 escrowId, address recipient) external onlyAuthorized {
        EscrowDeposit storage escrow = escrowDeposits[escrowId];
        require(escrow.user != address(0), "Escrow does not exist");
        require(!escrow.released && !escrow.refunded, "Escrow already processed");
        require(recipient != address(0), "Invalid recipient");
        
        escrow.released = true;
        escrowBalances[escrow.user] -= escrow.amount;
        totalEscrow -= escrow.amount;
        
        if (recipient == address(this)) {
            // Keep in contract for platform fees
        } else {
            balances[recipient] += escrow.amount;
        }
        
        emit EscrowReleased(escrowId, escrow.user, escrow.amount);
    }
    
    function refundEscrow(bytes32 escrowId) external {
        EscrowDeposit storage escrow = escrowDeposits[escrowId];
        require(escrow.user != address(0), "Escrow does not exist");
        require(!escrow.released && !escrow.refunded, "Escrow already processed");
        require(
            msg.sender == escrow.user || 
            msg.sender == owner || 
            block.timestamp > escrow.timeout,
            "Not authorized to refund"
        );
        
        escrow.refunded = true;
        escrowBalances[escrow.user] -= escrow.amount;
        balances[escrow.user] += escrow.amount;
        totalEscrow -= escrow.amount;
        
        emit EscrowRefunded(escrowId, escrow.user, escrow.amount);
    }
    
    // Payment processing
    function processPayment(
        address from,
        address to,
        uint256 amount,
        bytes32 transactionId
    ) external onlyAuthorized sufficientBalance(from, amount) {
        require(to != address(0), "Invalid recipient");
        
        balances[from] -= amount;
        
        if (to == address(this)) {
            // Platform fee - keep in contract
        } else {
            balances[to] += amount;
        }
        
        emit PaymentProcessed(from, to, amount, transactionId);
    }
    
    function batchProcessPayments(
        address[] memory from,
        address[] memory to,
        uint256[] memory amounts,
        bytes32[] memory transactionIds
    ) external onlyAuthorized {
        require(
            from.length == to.length && 
            to.length == amounts.length && 
            amounts.length == transactionIds.length,
            "Array lengths mismatch"
        );
        
        for (uint256 i = 0; i < from.length; i++) {
            require(balances[from[i]] >= amounts[i], "Insufficient balance");
            require(to[i] != address(0), "Invalid recipient");
            
            balances[from[i]] -= amounts[i];
            
            if (to[i] != address(this)) {
                balances[to[i]] += amounts[i];
            }
            
            emit PaymentProcessed(from[i], to[i], amounts[i], transactionIds[i]);
        }
    }
    
    // Administrative functions
    function authorizeContract(address contractAddress, bool authorized) external onlyOwner {
        require(contractAddress != address(0), "Invalid contract address");
        authorizedContracts[contractAddress] = authorized;
        emit ContractAuthorized(contractAddress, authorized);
    }
    
    function setBillingContract(address _billingContract) external onlyOwner {
        require(_billingContract != address(0), "Invalid billing contract");
        billingContract = _billingContract;
        authorizedContracts[_billingContract] = true;
    }
    
    function setMeteringContract(address _meteringContract) external onlyOwner {
        require(_meteringContract != address(0), "Invalid metering contract");
        meteringContract = _meteringContract;
        authorizedContracts[_meteringContract] = true;
    }
    
    function setMinimumDeposit(uint256 _minimumDeposit) external onlyOwner {
        minimumDeposit = _minimumDeposit;
    }
    
    function setEscrowTimeout(uint256 _escrowTimeout) external onlyOwner {
        require(_escrowTimeout >= 1 hours, "Timeout too short");
        require(_escrowTimeout <= 30 days, "Timeout too long");
        escrowTimeout = _escrowTimeout;
    }
    
    function transferOwnership(address newOwner) external onlyOwner {
        require(newOwner != address(0), "Invalid new owner");
        require(newOwner != owner, "Same owner");
        
        address previousOwner = owner;
        owner = newOwner;
        authorizedContracts[newOwner] = true;
        
        emit OwnershipTransferred(previousOwner, newOwner);
    }
    
    // Emergency functions
    function emergencyWithdraw() external onlyOwner {
        uint256 contractBalance = address(this).balance;
        require(contractBalance > 0, "No balance to withdraw");
        
        (bool success, ) = owner.call{value: contractBalance}("");
        require(success, "Emergency withdrawal failed");
    }
    
    function pause() external onlyOwner {
        // Implementation for pausing contract functionality
        // This would require additional state variables and modifiers
    }
    
    // View functions
    function getBalance(address user) external view returns (uint256) {
        return balances[user];
    }
    
    function getEscrowBalance(address user) external view returns (uint256) {
        return escrowBalances[user];
    }
    
    function getEscrowDetails(bytes32 escrowId) external view returns (
        address user,
        uint256 amount,
        uint256 createdAt,
        uint256 timeout,
        bool released,
        bool refunded,
        string memory purpose
    ) {
        EscrowDeposit memory escrow = escrowDeposits[escrowId];
        return (
            escrow.user,
            escrow.amount,
            escrow.createdAt,
            escrow.timeout,
            escrow.released,
            escrow.refunded,
            escrow.purpose
        );
    }
    
    function getTotalStats() external view returns (
        uint256 _totalDeposits,
        uint256 _totalEscrow,
        uint256 _contractBalance
    ) {
        return (totalDeposits, totalEscrow, address(this).balance);
    }
    
    function isAuthorized(address contractAddress) external view returns (bool) {
        return authorizedContracts[contractAddress];
    }
    
    // Fallback function to receive Ether
    receive() external payable {
        if (msg.value > 0) {
            balances[msg.sender] += msg.value;
            totalDeposits += msg.value;
            emit Deposit(msg.sender, msg.value, block.timestamp);
        }
    }
}