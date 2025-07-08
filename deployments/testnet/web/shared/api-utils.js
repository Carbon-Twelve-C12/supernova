// Shared API utilities for Supernova testnet web interfaces

const API_CONFIG = {
    baseUrl: 'http://testnet.supernovanetwork.xyz:8332/api/v1',
    timeout: 10000, // 10 seconds
    retryAttempts: 3,
    retryDelay: 1000, // 1 second base delay
    exponentialBackoff: true
};

/**
 * Sleep for specified milliseconds
 */
function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * Fetch with timeout
 */
async function fetchWithTimeout(url, options = {}, timeout = API_CONFIG.timeout) {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeout);
    
    try {
        const response = await fetch(url, {
            ...options,
            signal: controller.signal
        });
        clearTimeout(timeoutId);
        return response;
    } catch (error) {
        clearTimeout(timeoutId);
        if (error.name === 'AbortError') {
            throw new Error('Request timeout');
        }
        throw error;
    }
}

/**
 * Make API call with retry logic
 */
async function apiCall(endpoint, options = {}) {
    const url = endpoint.startsWith('http') ? endpoint : `${API_CONFIG.baseUrl}${endpoint}`;
    let lastError;
    
    for (let attempt = 0; attempt < API_CONFIG.retryAttempts; attempt++) {
        try {
            const response = await fetchWithTimeout(url, options);
            
            // Check if response is ok
            if (!response.ok) {
                // Don't retry on client errors (4xx)
                if (response.status >= 400 && response.status < 500) {
                    const errorData = await response.json().catch(() => ({}));
                    throw new Error(errorData.message || errorData.error || `HTTP ${response.status} error`);
                }
                // Retry on server errors (5xx)
                throw new Error(`Server error: ${response.status}`);
            }
            
            return await response.json();
        } catch (error) {
            lastError = error;
            
            // Don't retry on client errors
            if (error.message && error.message.includes('HTTP 4')) {
                throw error;
            }
            
            // Calculate retry delay
            if (attempt < API_CONFIG.retryAttempts - 1) {
                const delay = API_CONFIG.exponentialBackoff 
                    ? API_CONFIG.retryDelay * Math.pow(2, attempt)
                    : API_CONFIG.retryDelay;
                    
                console.warn(`API call failed (attempt ${attempt + 1}/${API_CONFIG.retryAttempts}), retrying in ${delay}ms...`, error);
                await sleep(delay);
            }
        }
    }
    
    throw lastError || new Error('API call failed after all retry attempts');
}

/**
 * Format large numbers with commas
 */
function formatNumber(num) {
    if (num === null || num === undefined) return '0';
    return new Intl.NumberFormat().format(num);
}

/**
 * Format amount from satoshis to NOVA
 */
function formatAmount(satoshis) {
    if (!satoshis) return '0.00000000';
    return (satoshis / 100000000).toFixed(8);
}

/**
 * Format hashrate with appropriate units
 */
function formatHashrate(hashrate) {
    if (!hashrate || hashrate === 0) return '0 H/s';
    
    const units = ['H/s', 'KH/s', 'MH/s', 'GH/s', 'TH/s', 'PH/s'];
    let unitIndex = 0;
    let value = hashrate;
    
    while (value >= 1000 && unitIndex < units.length - 1) {
        value /= 1000;
        unitIndex++;
    }
    
    return value.toFixed(2) + ' ' + units[unitIndex];
}

/**
 * Format timestamp to relative time
 */
function formatRelativeTime(timestamp) {
    if (!timestamp) return 'Unknown';
    
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now - date;
    const diffMins = Math.floor(diffMs / 60000);
    
    if (diffMins < 1) return 'just now';
    if (diffMins < 60) return `${diffMins} min ago`;
    
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours} hour${diffHours > 1 ? 's' : ''} ago`;
    
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 30) return `${diffDays} day${diffDays > 1 ? 's' : ''} ago`;
    
    const diffMonths = Math.floor(diffDays / 30);
    return `${diffMonths} month${diffMonths > 1 ? 's' : ''} ago`;
}

/**
 * Format address for display (shortened)
 */
function formatAddress(address, prefixLength = 12, suffixLength = 8) {
    if (!address || address.length < prefixLength + suffixLength) return address || 'N/A';
    return `${address.substring(0, prefixLength)}...${address.substring(address.length - suffixLength)}`;
}

/**
 * Show error message in UI
 */
function showError(message, elementId = 'errorMessage', duration = 5000) {
    const errorElement = document.getElementById(elementId);
    if (!errorElement) {
        console.error('Error element not found:', elementId);
        console.error('Error message:', message);
        return;
    }
    
    errorElement.textContent = message;
    errorElement.style.display = 'block';
    
    if (duration > 0) {
        setTimeout(() => {
            errorElement.style.display = 'none';
        }, duration);
    }
}

/**
 * Show success message in UI
 */
function showSuccess(message, elementId = 'successMessage', duration = 3000) {
    const successElement = document.getElementById(elementId);
    if (!successElement) {
        console.log('Success:', message);
        return;
    }
    
    successElement.textContent = message;
    successElement.style.display = 'block';
    
    if (duration > 0) {
        setTimeout(() => {
            successElement.style.display = 'none';
        }, duration);
    }
}

/**
 * Create a connection status monitor
 */
class ConnectionMonitor {
    constructor(statusElementId = 'connectionStatus', checkInterval = 30000) {
        this.statusElementId = statusElementId;
        this.checkInterval = checkInterval;
        this.isConnected = false;
        this.intervalId = null;
    }
    
    async checkConnection() {
        try {
            const response = await apiCall('/blockchain/info');
            this.updateStatus(true);
            return true;
        } catch (error) {
            this.updateStatus(false);
            return false;
        }
    }
    
    updateStatus(connected) {
        this.isConnected = connected;
        const element = document.getElementById(this.statusElementId);
        if (element) {
            element.classList.toggle('connected', connected);
            element.classList.toggle('disconnected', !connected);
            element.textContent = connected ? 'Connected' : 'Disconnected';
        }
    }
    
    start() {
        // Initial check
        this.checkConnection();
        
        // Periodic checks
        this.intervalId = setInterval(() => {
            this.checkConnection();
        }, this.checkInterval);
    }
    
    stop() {
        if (this.intervalId) {
            clearInterval(this.intervalId);
            this.intervalId = null;
        }
    }
}

// Export for use in other scripts
if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
        API_CONFIG,
        apiCall,
        formatNumber,
        formatAmount,
        formatHashrate,
        formatRelativeTime,
        formatAddress,
        showError,
        showSuccess,
        ConnectionMonitor
    };
} 