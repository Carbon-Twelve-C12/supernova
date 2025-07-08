# Supernova Testnet Web Interfaces

This directory contains the web interfaces for the Supernova testnet, providing user-friendly access to blockchain functionality.

## Web Interfaces

### 1. Block Explorer (`/explorer`)
- View blockchain statistics
- Browse recent blocks and transactions
- Search by block height, transaction hash, or address
- Real-time updates of network activity

### 2. Faucet (`/faucet`)
- Request testnet NOVA tokens
- View recent faucet transactions
- Rate-limited distribution with CAPTCHA protection
- Automatic balance checking

### 3. Wallet (`/wallet`)
- Connect to testnet wallet
- View balance (total, confirmed, unconfirmed, spendable)
- Generate receiving addresses
- Basic wallet functionality

### 4. Network Status (`/status`)
- Real-time network metrics
- Node connectivity information
- Environmental impact tracking
- Network activity visualization

### 5. API Documentation (`/api`)
- Interactive API documentation
- Endpoint descriptions and examples

### 6. Grafana Dashboard (`/grafana`)
- Advanced metrics and monitoring
- Performance visualization

### 7. Landing Page (`/landing`)
- Testnet overview and links

## Shared API Utilities

All web interfaces use the shared API utilities located in `/shared/api-utils.js` for consistent error handling and retry logic.

### Features

- **Automatic Retry Logic**: Failed requests are retried up to 3 times with exponential backoff
- **Timeout Protection**: Requests timeout after 10 seconds to prevent hanging
- **Connection Monitoring**: Automatic connection status checking
- **Consistent Formatting**: Shared utilities for formatting numbers, amounts, hashrates, and addresses
- **Error Handling**: Unified error display across all interfaces

### Usage Example

```html
<!-- Include the shared utilities -->
<script src="../shared/api-utils.js"></script>

<script>
// Make API calls with automatic retry
async function fetchData() {
    try {
        const info = await apiCall('/blockchain/info');
        console.log('Blockchain height:', info.height);
    } catch (error) {
        showError('Failed to fetch blockchain info: ' + error.message);
    }
}

// Use formatting utilities
const formatted = formatAmount(123456789); // "1.23456789"
const hashrate = formatHashrate(1234567890); // "1.23 GH/s"

// Monitor connection status
const monitor = new ConnectionMonitor('connectionStatus');
monitor.start();
</script>
```

## API Configuration

The API base URL is configured in `api-utils.js`:
```javascript
const API_CONFIG = {
    baseUrl: 'http://testnet.supernovanetwork.xyz:8332/api/v1',
    timeout: 10000, // 10 seconds
    retryAttempts: 3,
    retryDelay: 1000, // 1 second base delay
    exponentialBackoff: true
};
```

## Error Handling

All interfaces implement consistent error handling:

1. **Network Errors**: Automatically retried with exponential backoff
2. **Timeout Errors**: Request cancelled after 10 seconds
3. **Client Errors (4xx)**: Not retried, error displayed to user
4. **Server Errors (5xx)**: Automatically retried

## Development

To test the web interfaces locally:

1. Ensure a Supernova testnet node is running
2. Serve the web directory with any HTTP server:
   ```bash
   cd deployments/testnet/web
   python3 -m http.server 8080
   ```
3. Access interfaces at `http://localhost:8080/[interface-name]`

## Production Deployment

In production, these interfaces are served by the testnet infrastructure:
- Explorer: http://testnet.supernovanetwork.xyz:3001
- Faucet: http://testnet.supernovanetwork.xyz:3002
- Wallet: http://testnet.supernovanetwork.xyz:3003
- Status: http://testnet.supernovanetwork.xyz:3004

## Security Considerations

- All API calls use CORS headers for cross-origin requests
- Faucet implements rate limiting and CAPTCHA protection
- No private keys or sensitive data are stored in the browser
- All connections should use HTTPS in production 