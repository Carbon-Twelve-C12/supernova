document.addEventListener('DOMContentLoaded', function() {
    // Initialize variables
    const API_BASE_URL = '/api/faucet';
    const addressInput = document.getElementById('address');
    const addressValidationMsg = document.getElementById('address-validation-message');
    const faucetForm = document.getElementById('faucet-form');
    const submitBtn = document.getElementById('submit-btn');
    const btnText = document.querySelector('.btn-text');
    const btnLoading = document.querySelector('.btn-loading');
    const successMessage = document.getElementById('success-message');
    const errorMessage = document.getElementById('error-message');
    const errorText = document.getElementById('error-text');
    const txIdElement = document.getElementById('tx-id');
    const copyBtn = document.querySelector('.copy-btn');
    const faucetBalance = document.getElementById('faucet-balance');
    const txCount = document.getElementById('tx-count');
    const lastDistribution = document.getElementById('last-distribution');
    const transactionsBody = document.getElementById('transactions-body');
    
    // Add simple captcha (will be replaced with a proper captcha service in production)
    initCaptcha();
    
    // Get initial faucet status
    fetchFaucetStatus();
    
    // Get recent transactions
    fetchRecentTransactions();
    
    // Setup event listeners
    addressInput.addEventListener('input', validateAddress);
    faucetForm.addEventListener('submit', handleFormSubmit);
    
    if (copyBtn) {
        copyBtn.addEventListener('click', function(e) {
            e.preventDefault();
            copyToClipboard(txIdElement.textContent);
        });
    }
    
    // Update faucet status and transactions every 30 seconds
    setInterval(() => {
        fetchFaucetStatus();
        fetchRecentTransactions();
    }, 30000);
    
    /**
     * Validate SuperNova testnet address
     */
    function validateAddress() {
        const address = addressInput.value.trim();
        
        // Simple validation for testnet - check for "test1" prefix and length
        if (address.length === 0) {
            addressValidationMsg.textContent = '';
            addressValidationMsg.className = '';
            return false;
        } else if (address.startsWith('test1') && address.length >= 40) {
            addressValidationMsg.textContent = '✓ Valid testnet address';
            addressValidationMsg.className = 'valid';
            return true;
        } else {
            addressValidationMsg.textContent = '✗ Invalid testnet address format';
            addressValidationMsg.className = 'invalid';
            return false;
        }
    }
    
    /**
     * Handle form submission
     */
    async function handleFormSubmit(e) {
        e.preventDefault();
        
        // Hide previous messages
        successMessage.style.display = 'none';
        errorMessage.style.display = 'none';
        
        // Validate address
        if (!validateAddress()) {
            showError('Please enter a valid SuperNova testnet address.');
            return;
        }
        
        // Validate captcha
        if (!validateCaptcha()) {
            showError('Please complete the captcha verification.');
            return;
        }
        
        // Show loading state
        setLoading(true);
        
        // Get form data
        const address = addressInput.value.trim();
        
        try {
            // Send request to the faucet API
            const response = await fetch(`${API_BASE_URL}/send`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ address })
            });
            
            const data = await response.json();
            
            if (!response.ok) {
                throw new Error(data.message || 'Failed to request funds from faucet.');
            }
            
            // Show success message
            txIdElement.textContent = data.txid;
            successMessage.style.display = 'flex';
            
            // Reset form
            faucetForm.reset();
            initCaptcha();
            
            // Refresh faucet status and transactions
            fetchFaucetStatus();
            fetchRecentTransactions();
            
        } catch (error) {
            showError(error.message || 'An error occurred. Please try again later.');
        } finally {
            setLoading(false);
        }
    }
    
    /**
     * Show error message
     */
    function showError(message) {
        errorText.textContent = message;
        errorMessage.style.display = 'flex';
    }
    
    /**
     * Set loading state
     */
    function setLoading(isLoading) {
        if (isLoading) {
            submitBtn.disabled = true;
            btnText.style.display = 'none';
            btnLoading.style.display = 'inline-block';
        } else {
            submitBtn.disabled = false;
            btnText.style.display = 'inline-block';
            btnLoading.style.display = 'none';
        }
    }
    
    /**
     * Copy text to clipboard
     */
    function copyToClipboard(text) {
        navigator.clipboard.writeText(text)
            .then(() => {
                const originalText = copyBtn.innerHTML;
                copyBtn.innerHTML = '<i class="fas fa-check"></i>';
                setTimeout(() => {
                    copyBtn.innerHTML = originalText;
                }, 2000);
            })
            .catch(err => {
                console.error('Failed to copy: ', err);
            });
    }
    
    /**
     * Fetch faucet status
     */
    async function fetchFaucetStatus() {
        try {
            const response = await fetch(`${API_BASE_URL}/status`);
            
            if (!response.ok) {
                throw new Error('Failed to fetch faucet status');
            }
            
            const data = await response.json();
            
            // Update status information
            faucetBalance.textContent = formatNovaAmount(data.balance);
            txCount.textContent = data.transactions_today;
            lastDistribution.textContent = data.last_distribution ? formatTimeAgo(new Date(data.last_distribution)) : 'N/A';
            
            // Update network status indicator
            const statusIndicator = document.querySelector('.status-indicator');
            if (data.is_online) {
                statusIndicator.classList.add('online');
                statusIndicator.classList.remove('offline');
            } else {
                statusIndicator.classList.add('offline');
                statusIndicator.classList.remove('online');
            }
            
        } catch (error) {
            console.error('Error fetching faucet status:', error);
        }
    }
    
    /**
     * Fetch recent transactions
     */
    async function fetchRecentTransactions() {
        try {
            const response = await fetch(`${API_BASE_URL}/transactions`);
            
            if (!response.ok) {
                throw new Error('Failed to fetch recent transactions');
            }
            
            const data = await response.json();
            
            // Clear loading message
            transactionsBody.innerHTML = '';
            
            // Update transactions table
            if (data.transactions && data.transactions.length > 0) {
                data.transactions.forEach(tx => {
                    const row = document.createElement('tr');
                    
                    // Format timestamp
                    const time = document.createElement('td');
                    const txTime = new Date(tx.timestamp);
                    time.textContent = formatTimeAgo(txTime);
                    time.title = txTime.toLocaleString();
                    
                    // Format address (truncate long addresses)
                    const address = document.createElement('td');
                    address.textContent = formatAddress(tx.recipient);
                    address.title = tx.recipient;
                    
                    // Format amount
                    const amount = document.createElement('td');
                    amount.textContent = formatNovaAmount(tx.amount);
                    
                    // Format transaction ID with link
                    const txid = document.createElement('td');
                    txid.className = 'txid-cell';
                    
                    const txLink = document.createElement('a');
                    txLink.href = `https://testnet-explorer.supernova.xyz/tx/${tx.txid}`;
                    txLink.target = '_blank';
                    txLink.textContent = formatTxId(tx.txid);
                    txLink.title = tx.txid;
                    
                    txid.appendChild(txLink);
                    
                    // Add cells to row
                    row.appendChild(time);
                    row.appendChild(address);
                    row.appendChild(amount);
                    row.appendChild(txid);
                    
                    // Add row to table
                    transactionsBody.appendChild(row);
                });
            } else {
                // No transactions yet
                const row = document.createElement('tr');
                const cell = document.createElement('td');
                cell.colSpan = 4;
                cell.className = 'loading-row';
                cell.textContent = 'No transactions yet';
                row.appendChild(cell);
                transactionsBody.appendChild(row);
            }
            
        } catch (error) {
            console.error('Error fetching recent transactions:', error);
            
            // Show error message
            const row = document.createElement('tr');
            const cell = document.createElement('td');
            cell.colSpan = 4;
            cell.className = 'loading-row';
            cell.textContent = 'Error loading transactions';
            row.appendChild(cell);
            transactionsBody.innerHTML = '';
            transactionsBody.appendChild(row);
        }
    }
    
    /**
     * Format NOVA amount
     */
    function formatNovaAmount(amount) {
        // Convert from nanoNOVA to NOVA
        const nova = amount / 100000000;
        return `${nova.toLocaleString()} NOVA`;
    }
    
    /**
     * Format time ago
     */
    function formatTimeAgo(date) {
        const now = new Date();
        const diffInSeconds = Math.floor((now - date) / 1000);
        
        if (diffInSeconds < 60) {
            return `${diffInSeconds} sec ago`;
        } else if (diffInSeconds < 3600) {
            return `${Math.floor(diffInSeconds / 60)} min ago`;
        } else if (diffInSeconds < 86400) {
            return `${Math.floor(diffInSeconds / 3600)} hr ago`;
        } else {
            return `${Math.floor(diffInSeconds / 86400)} day ago`;
        }
    }
    
    /**
     * Format address (truncate long addresses)
     */
    function formatAddress(address) {
        if (address.length <= 14) return address;
        return `${address.substring(0, 8)}...${address.substring(address.length - 6)}`;
    }
    
    /**
     * Format transaction ID
     */
    function formatTxId(txid) {
        return `${txid.substring(0, 8)}...${txid.substring(txid.length - 8)}`;
    }
    
    /**
     * Initialize simple captcha
     * Note: This is a simple implementation for development.
     * In production, use a proper captcha service like reCAPTCHA.
     */
    function initCaptcha() {
        const captchaContainer = document.getElementById('captcha');
        const num1 = Math.floor(Math.random() * 10) + 1;
        const num2 = Math.floor(Math.random() * 10) + 1;
        const sum = num1 + num2;
        
        captchaContainer.innerHTML = `
            <div class="simple-captcha">
                <span>${num1} + ${num2} = ?</span>
                <input type="number" id="captcha-answer" min="0" max="20" required>
            </div>
        `;
        
        // Store the correct answer in a data attribute
        captchaContainer.dataset.correctAnswer = sum.toString();
    }
    
    /**
     * Validate captcha
     */
    function validateCaptcha() {
        const captchaContainer = document.getElementById('captcha');
        const captchaInput = document.getElementById('captcha-answer');
        const correctAnswer = captchaContainer.dataset.correctAnswer;
        
        return captchaInput.value === correctAnswer;
    }
}); 