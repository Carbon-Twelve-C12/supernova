document.addEventListener('DOMContentLoaded', function() {
    // Initialize variables
    const API_BASE_URL = 'http://testnet.supernovanetwork.xyz:8332/api/v1/faucet';
    const addressInput = document.getElementById('address');
    const faucetForm = document.getElementById('faucetForm');
    const recentTransactions = document.getElementById('recent-transactions');
    const resultDiv = document.getElementById('result');
    
    // Initialize captcha
    initCaptcha();
    
    // Load recent transactions
    loadRecentTransactions();
    
    // Set up form submission
    faucetForm.addEventListener('submit', handleFormSubmit);
    
    // Refresh recent transactions every 30 seconds
    setInterval(loadRecentTransactions, 30000);
    
    /**
     * Initialize simple captcha
     */
    function initCaptcha() {
        const captchaContainer = document.getElementById('captcha');
        const num1 = Math.floor(Math.random() * 10) + 1;
        const num2 = Math.floor(Math.random() * 10) + 1;
        const sum = num1 + num2;
        
        captchaContainer.innerHTML = `
            <label for="captcha-answer">Security Check: What is ${num1} + ${num2}?</label>
            <input type="number" id="captcha-answer" name="captcha" min="0" max="20" required>
        `;
        
        // Store the correct answer
        captchaContainer.dataset.correctAnswer = sum.toString();
    }
    
    /**
     * Handle form submission
     */
    async function handleFormSubmit(e) {
        e.preventDefault();
        
        const resultDiv = document.getElementById('result');
        const address = document.getElementById('address').value.trim();
        const amount = document.getElementById('amount').value;
        const captchaInput = document.getElementById('captcha-answer');
        const captchaContainer = document.getElementById('captcha');
        
        // Hide previous results
        resultDiv.classList.add('hidden');
        resultDiv.classList.remove('success', 'error');
        
        // Validate captcha
        if (captchaInput.value !== captchaContainer.dataset.correctAnswer) {
            showResult('error', 'Incorrect captcha answer. Please try again.');
            initCaptcha();
            return;
        }
        
        // Show loading state
        const submitBtn = faucetForm.querySelector('button[type="submit"]');
        const originalText = submitBtn.textContent;
        submitBtn.disabled = true;
        submitBtn.innerHTML = '<span class="loading"></span> Requesting...';
        
        try {
            // Make API call to request tokens
            const response = await fetch(`${API_BASE_URL}/send`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    address: address
                })
            });
            
            const data = await response.json();
            
            if (response.ok) {
                // Success response
                showResult('success', `
                    Success! ${formatAmount(data.amount)} NOVA sent to ${formatAddress(data.recipient)}
                    <br><br>
                    Transaction ID: <code>${data.txid}</code>
                `);
                
                // Reset form
                faucetForm.reset();
                initCaptcha();
                
                // Refresh transactions
                setTimeout(loadRecentTransactions, 1000);
            } else {
                // Handle various error cases
                let errorMessage = 'Failed to send tokens.';
                
                if (response.status === 429) {
                    errorMessage = data.message || 'Rate limit exceeded. Please try again later.';
                } else if (response.status === 400) {
                    errorMessage = data.message || 'Invalid address format.';
                } else if (response.status === 503) {
                    errorMessage = 'Faucet is temporarily unavailable. Please try again later.';
                } else if (data && data.message) {
                    errorMessage = data.message;
                }
                
                showResult('error', errorMessage);
            }
            
        } catch (error) {
            console.error('Faucet request error:', error);
            showResult('error', 'Network error. Please check your connection and try again.');
        } finally {
            submitBtn.disabled = false;
            submitBtn.textContent = originalText;
        }
    }
    
    /**
     * Show result message
     */
    function showResult(type, message) {
        const resultDiv = document.getElementById('result');
        resultDiv.innerHTML = message;
        resultDiv.classList.remove('hidden', 'success', 'error');
        resultDiv.classList.add(type);
    }
    
    /**
     * Format amount from satoshis to NOVA
     */
    function formatAmount(satoshis) {
        return (satoshis / 100000000).toFixed(8);
    }
    
    /**
     * Format address for display
     */
    function formatAddress(address) {
        if (!address || address.length < 12) return address;
        return `${address.substring(0, 12)}...${address.substring(address.length - 8)}`;
    }
    
    /**
     * Format timestamp to relative time
     */
    function formatTime(timestamp) {
        const date = new Date(timestamp);
        const now = new Date();
        const diffMs = now - date;
        const diffMins = Math.floor(diffMs / 60000);
        
        if (diffMins < 1) return 'just now';
        if (diffMins < 60) return `${diffMins} min ago`;
        
        const diffHours = Math.floor(diffMins / 60);
        if (diffHours < 24) return `${diffHours} hour${diffHours > 1 ? 's' : ''} ago`;
        
        const diffDays = Math.floor(diffHours / 24);
        return `${diffDays} day${diffDays > 1 ? 's' : ''} ago`;
    }
    
    /**
     * Load recent transactions
     */
    async function loadRecentTransactions() {
        const tbody = document.getElementById('recent-transactions');
        
        try {
            // Fetch recent transactions from API
            const response = await fetch(`${API_BASE_URL}/transactions`);
            
            if (response.ok) {
                const data = await response.json();
                const transactions = data.transactions || [];
                
                if (transactions.length > 0) {
                    tbody.innerHTML = transactions.slice(0, 5).map(tx => `
                        <tr>
                            <td>${formatTime(tx.timestamp)}</td>
                            <td>${formatAddress(tx.recipient)}</td>
                            <td>${formatAmount(tx.amount)} NOVA</td>
                            <td><a href="/explorer/tx/${tx.txid}" style="color: #00ff88;">${tx.txid.substring(0, 8)}...</a></td>
                        </tr>
                    `).join('');
                } else {
                    tbody.innerHTML = '<tr><td colspan="4" style="text-align: center; color: #8899a6;">No recent transactions</td></tr>';
                }
            } else {
                throw new Error('Failed to fetch transactions');
            }
            
        } catch (error) {
            console.error('Error loading recent transactions:', error);
            tbody.innerHTML = '<tr><td colspan="4" style="text-align: center; color: #8899a6;">Unable to load recent transactions</td></tr>';
        }
    }
    
    // Load faucet status on page load
    async function loadFaucetStatus() {
        try {
            const response = await fetch(`${API_BASE_URL}/status`);
            
            if (response.ok) {
                const status = await response.json();
                
                // Update UI based on faucet status
                if (!status.is_online) {
                    showResult('error', 'Faucet is currently offline. Please try again later.');
                    faucetForm.querySelector('button[type="submit"]').disabled = true;
                }
                
                // Could also display balance, cooldown period, etc.
            }
        } catch (error) {
            console.error('Error loading faucet status:', error);
        }
    }
    
    // Load faucet status on page load
    loadFaucetStatus();
}); 