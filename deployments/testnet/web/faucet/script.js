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
            // Simulate API call (replace with actual API endpoint)
            await new Promise(resolve => setTimeout(resolve, 2000));
            
            // Mock transaction ID
            const txId = '0x' + Math.random().toString(16).substr(2, 64);
            
            showResult('success', `
                Success! ${amount} NOVA sent to ${address.substring(0, 12)}...${address.substring(address.length - 8)}
                <br><br>
                Transaction ID: <code>${txId}</code>
            `);
            
            // Reset form
            faucetForm.reset();
            initCaptcha();
            
            // Refresh transactions
            loadRecentTransactions();
            
        } catch (error) {
            showResult('error', 'Failed to send tokens. Please try again later.');
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
     * Load recent transactions
     */
    async function loadRecentTransactions() {
        const tbody = document.getElementById('recent-transactions');
        
        try {
            // Mock data for demonstration
            const mockTransactions = [
                { time: '2 min ago', address: 'nova1qxy...8dk3', amount: '10', tx: '0xa1b2...c3d4' },
                { time: '5 min ago', address: 'nova1abc...def9', amount: '25', tx: '0xe5f6...7890' },
                { time: '12 min ago', address: 'nova1ghi...jkl2', amount: '50', tx: '0xm3n4...o5p6' },
                { time: '23 min ago', address: 'nova1mno...pqr7', amount: '15', tx: '0xq7r8...s9t0' },
                { time: '45 min ago', address: 'nova1stu...vwx4', amount: '30', tx: '0xu1v2...w3x4' }
            ];
            
            tbody.innerHTML = mockTransactions.map(tx => `
                <tr>
                    <td>${tx.time}</td>
                    <td>${tx.address}</td>
                    <td>${tx.amount} NOVA</td>
                    <td><a href="#" style="color: #00ff88;">${tx.tx}</a></td>
                </tr>
            `).join('');
            
        } catch (error) {
            tbody.innerHTML = '<tr><td colspan="4" style="text-align: center; color: #8899a6;">Unable to load recent transactions</td></tr>';
        }
    }
}); 