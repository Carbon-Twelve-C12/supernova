# Supernova Frontend Integration Guide

## Overview

This guide provides step-by-step instructions for integrating the Supernova blockchain testnet API with the frontend web application at https://github.com/mjohnson518/supernova-web.

**Last Updated**: September 30, 2025  
**Backend Version**: 1.0.0-RC4  
**Frontend Repo**: https://github.com/mjohnson518/supernova-web  
**Backend Repo**: https://github.com/Carbon-Twelve-C12/supernova

---

## Backend API Status ✅

**STATUS**: JSON-RPC API fully implemented and operational

- **Endpoint**: `http://localhost:8332`
- **Protocol**: JSON-RPC 2.0
- **Authentication**: Bearer token
- **CORS**: Enabled
- **Methods**: 20+ methods implemented

See `RPC_IMPLEMENTATION.md` for full API documentation.

---

## Frontend Repository Updates

### 1. Environment Configuration

Create or update `.env.local` in the frontend repo:

```bash
# Testnet API Configuration
NEXT_PUBLIC_TESTNET_API_URL=http://localhost:8332
NEXT_PUBLIC_TESTNET_WS_URL=ws://localhost:8332/ws
NEXT_PUBLIC_NETWORK_ID=supernova-testnet
NEXT_PUBLIC_CHAIN_ID=supernova-testnet
NEXT_PUBLIC_GENESIS_HASH=00000000e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b

# Production API Configuration (when deployed)
# NEXT_PUBLIC_API_URL=https://api.supernovanetwork.xyz
# NEXT_PUBLIC_WS_URL=wss://api.supernovanetwork.xyz/ws

# API Authentication
NEXT_PUBLIC_API_KEY=supernova-testnet-dev-key-2024

# Optional: Analytics
NEXT_PUBLIC_ENABLE_ANALYTICS=false
```

### 2. Install Dependencies

Add to `package.json`:

```json
{
  "dependencies": {
    "axios": "^1.6.2",
    "swr": "^2.2.4",
    "date-fns": "^2.30.0",
    "recharts": "^2.10.3",
    "socket.io-client": "^4.6.1"
  }
}
```

Run:
```bash
npm install
```

### 3. Create API Client

Create `lib/supernova-client.ts`:

```typescript
import axios, { AxiosInstance } from 'axios';

export class SupernovaClient {
  private client: AxiosInstance;
  private requestId: number = 1;
  
  constructor() {
    this.client = axios.create({
      baseURL: process.env.NEXT_PUBLIC_TESTNET_API_URL,
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${process.env.NEXT_PUBLIC_API_KEY}`,
      },
      timeout: 10000,
    });
  }
  
  private async rpcCall(method: string, params: any[] = []): Promise<any> {
    const id = this.requestId++;
    
    try {
      const response = await this.client.post('', {
        jsonrpc: '2.0',
        method,
        params,
        id,
      });
      
      if (response.data.error) {
        throw new Error(response.data.error.message);
      }
      
      return response.data.result;
    } catch (error) {
      if (axios.isAxiosError(error)) {
        throw new Error(`RPC call failed: ${error.message}`);
      }
      throw error;
    }
  }
  
  // Blockchain Methods
  async getBlockCount(): Promise<number> {
    return this.rpcCall('getblockcount');
  }
  
  async getBestBlockHash(): Promise<string> {
    return this.rpcCall('getbestblockhash');
  }
  
  async getBlock(hashOrHeight: string | number): Promise<any> {
    if (typeof hashOrHeight === 'number') {
      const hash = await this.rpcCall('getblockhash', [hashOrHeight]);
      return this.rpcCall('getblock', { blockhash: hash, verbosity: 2 });
    }
    return this.rpcCall('getblock', { blockhash: hashOrHeight, verbosity: 2 });
  }
  
  async getBlockchainInfo(): Promise<any> {
    return this.rpcCall('getblockchaininfo');
  }
  
  // Network Methods
  async getNetworkStats(): Promise<any> {
    return this.rpcCall('getnetworkstats');
  }
  
  async getNetworkInfo(): Promise<any> {
    return this.rpcCall('getnetworkinfo');
  }
  
  async getPeerInfo(): Promise<any> {
    return this.rpcCall('getpeerinfo');
  }
  
  // Environmental Methods
  async getEnvironmentalMetrics(): Promise<any> {
    return this.rpcCall('getenvironmentalmetrics');
  }
  
  async getEnvironmentalInfo(): Promise<any> {
    return this.rpcCall('getenvironmentalinfo');
  }
  
  // Mining Methods
  async getMiningInfo(): Promise<any> {
    return this.rpcCall('getmininginfo');
  }
  
  // Mempool Methods
  async getMempoolInfo(): Promise<any> {
    return this.rpcCall('getmempoolinfo');
  }
  
  async getRawMempool(verbose: boolean = false): Promise<any> {
    return this.rpcCall('getrawmempool', [verbose]);
  }
}

export const supernovaClient = new SupernovaClient();
```

### 4. Create React Hooks

Create `hooks/useSupernova.ts`:

```typescript
import useSWR from 'swr';
import { supernovaClient } from '@/lib/supernova-client';

export function useBlockchainData() {
  const { data: blockCount, error: blockCountError } = useSWR(
    'blockCount',
    () => supernovaClient.getBlockCount(),
    { refreshInterval: 10000 } // Refresh every 10s
  );
  
  const { data: networkStats, error: networkError } = useSWR(
    'networkStats',
    () => supernovaClient.getNetworkStats(),
    { refreshInterval: 10000 }
  );
  
  const { data: environmentalMetrics, error: envError } = useSWR(
    'environmentalMetrics',
    () => supernovaClient.getEnvironmentalMetrics(),
    { refreshInterval: 30000 } // Refresh every 30s
  );
  
  return {
    blockCount,
    networkStats,
    environmentalMetrics,
    loading: !blockCount && !blockCountError,
    error: blockCountError || networkError || envError,
  };
}

export function useLatestBlocks(count: number = 10) {
  const { data, error } = useSWR(
    ['latestBlocks', count],
    async () => {
      const height = await supernovaClient.getBlockCount();
      const blocks = [];
      
      for (let i = 0; i < count && i <= height; i++) {
        const block = await supernovaClient.getBlock(height - i);
        blocks.push(block);
      }
      
      return blocks;
    },
    { refreshInterval: 15000 }
  );
  
  return {
    blocks: data || [],
    loading: !data && !error,
    error,
  };
}
```

### 5. Example Dashboard Component

Create or update `pages/testnet/index.tsx`:

```tsx
import React from 'react';
import { useBlockchainData, useLatestBlocks } from '@/hooks/useSupernova';
import { Box, Grid, Card, Typography, CircularProgress, Alert } from '@mui/material';

export default function TestnetDashboard() {
  const { blockCount, networkStats, environmentalMetrics, loading, error } = useBlockchainData();
  const { blocks, loading: blocksLoading } = useLatestBlocks(10);
  
  if (loading) {
    return (
      <Box display="flex" justifyContent="center" alignItems="center" minHeight="400px">
        <CircularProgress />
      </Box>
    );
  }
  
  if (error) {
    return (
      <Alert severity="error">
        Failed to connect to testnet: {error.message}
      </Alert>
    );
  }
  
  return (
    <Box p={4}>
      <Typography variant="h3" gutterBottom>
        Supernova Testnet Dashboard
      </Typography>
      
      {/* Network Statistics */}
      <Grid container spacing={3} mb={4}>
        <Grid item xs={12} md={3}>
          <Card sx={{ p: 3 }}>
            <Typography color="textSecondary" variant="caption">
              BLOCK HEIGHT
            </Typography>
            <Typography variant="h4">
              {networkStats?.blockHeight?.toLocaleString() || '0'}
            </Typography>
          </Card>
        </Grid>
        
        <Grid item xs={12} md={3}>
          <Card sx={{ p: 3 }}>
            <Typography color="textSecondary" variant="caption">
              NETWORK HASHRATE
            </Typography>
            <Typography variant="h5">
              {formatHashrate(networkStats?.hashrate || '0')}
            </Typography>
          </Card>
        </Grid>
        
        <Grid item xs={12} md={3}>
          <Card sx={{ p: 3 }}>
            <Typography color="textSecondary" variant="caption">
              CARBON STATUS
            </Typography>
            <Typography 
              variant="h5" 
              color={environmentalMetrics?.isCarbonNegative ? 'success.main' : 'warning.main'}
            >
              {environmentalMetrics?.isCarbonNegative ? 'Negative' : 'Positive'}
            </Typography>
            <Typography variant="caption">
              {environmentalMetrics?.netCarbon?.toFixed(2)} tCO₂
            </Typography>
          </Card>
        </Grid>
        
        <Grid item xs={12} md={3}>
          <Card sx={{ p: 3 }}>
            <Typography color="textSecondary" variant="caption">
              GREEN MINING
            </Typography>
            <Typography variant="h4" color="success.main">
              {environmentalMetrics?.renewablePercentage?.toFixed(1) || '0'}%
            </Typography>
          </Card>
        </Grid>
      </Grid>
      
      {/* Latest Blocks */}
      <Typography variant="h5" gutterBottom>
        Latest Blocks
      </Typography>
      <Card>
        {blocks.map((block) => (
          <Box
            key={block.hash}
            p={2}
            borderBottom="1px solid"
            borderColor="divider"
          >
            <Grid container alignItems="center">
              <Grid item xs={2}>
                <Typography variant="h6">#{block.height}</Typography>
              </Grid>
              <Grid item xs={6}>
                <Typography variant="body2" fontFamily="monospace" noWrap>
                  {block.hash}
                </Typography>
              </Grid>
              <Grid item xs={2}>
                <Typography>{block.tx?.length || 0} txs</Typography>
              </Grid>
              <Grid item xs={2}>
                <Typography color="textSecondary">
                  {new Date(block.time * 1000).toLocaleTimeString()}
                </Typography>
              </Grid>
            </Grid>
          </Box>
        ))}
      </Card>
    </Box>
  );
}

function formatHashrate(hashrate: string): string {
  const value = parseFloat(hashrate);
  if (value >= 1e18) return `${(value / 1e18).toFixed(2)} EH/s`;
  if (value >= 1e15) return `${(value / 1e15).toFixed(2)} PH/s`;
  if (value >= 1e12) return `${(value / 1e12).toFixed(2)} TH/s`;
  if (value >= 1e9) return `${(value / 1e9).toFixed(2)} GH/s`;
  if (value >= 1e6) return `${(value / 1e6).toFixed(2)} MH/s`;
  return `${value.toFixed(2)} H/s`;
}
```

---

## Testing the Integration

### Step 1: Start Supernova Testnet Node

```bash
cd /path/to/supernova
./deployment/testnet-launch.sh
```

Verify node is running:
```bash
tail -f ./testnet.log
# Should show: "API server started on 127.0.0.1:8332"
```

### Step 2: Test API Manually

```bash
curl http://localhost:8332 \
  -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer supernova-testnet-dev-key-2024" \
  -d '{"jsonrpc":"2.0","method":"getblockcount","params":[],"id":1}'

# Expected: {"jsonrpc":"2.0","result":<height>,"id":1}
```

### Step 3: Start Frontend

```bash
cd /path/to/supernova-web
npm run dev
```

Visit: `http://localhost:3000/testnet`

### Step 4: Verify Data Flow

Check browser console for:
- Successful API calls (200 responses)
- Data updates every 10 seconds
- No CORS errors
- No authentication errors

---

## Common Integration Issues & Solutions

### Issue 1: CORS Errors

**Symptom**: `Access to fetch at 'http://localhost:8332' from origin 'http://localhost:3000' has been blocked by CORS policy`

**Solution**: Already fixed - CORS is enabled in `node/src/api/server.rs` (line 181-186)

### Issue 2: Authentication Errors

**Symptom**: `401 Unauthorized` or `{"error":{"code":-32001,"message":"Unauthorized"}}`

**Solution**: Ensure Bearer token matches:
- Backend: `config.toml` → `api_keys`
- Frontend: `.env.local` → `NEXT_PUBLIC_API_KEY`

### Issue 3: Connection Refused

**Symptom**: `ECONNREFUSED` or `Failed to connect to localhost port 8332`

**Solution**: 
1. Check node is running: `ps aux | grep supernova-node`
2. Check port is correct: `lsof -i :8332`
3. Restart node: `./deployment/testnet-launch.sh --clean`

### Issue 4: Stale Data

**Symptom**: Data doesn't update after initial load

**Solution**: Check SWR configuration in hooks has `refreshInterval` set

---

## API Method Mapping

| Frontend Need | Backend RPC Method | Response Field |
|---------------|-------------------|----------------|
| Current block height | `getblockcount` | `result` (number) |
| Latest block hash | `getbestblockhash` | `result` (string) |
| Block data | `getblock` | `result` (object) |
| Network hashrate | `getmininginfo` | `result.networkhashps` |
| Connected peers | `getnetworkinfo` | `result.connections` |
| Carbon footprint | `getenvironmentalmetrics` | `result.netCarbon` |
| Renewable % | `getenvironmentalmetrics` | `result.renewablePercentage` |
| Is carbon negative? | `getenvironmentalmetrics` | `result.isCarbonNegative` |
| Network stats (all) | `getnetworkstats` | `result` (object) |

---

## Production Deployment

### Backend Changes

1. **Update CORS** in `node/src/api/server.rs`:
```rust
.wrap(
    Cors::default()
        .allowed_origin("https://supernovanetwork.xyz")
        .allowed_origin("https://testnet.supernovanetwork.xyz")
        .allowed_origin("https://staging.supernovanetwork.xyz")
        .allowed_methods(vec!["POST", "OPTIONS"])
        .allowed_headers(vec!["Content-Type", "Authorization"])
        .max_age(3600)
)
```

2. **Generate secure API key**:
```bash
openssl rand -base64 32
# Add to config.toml: api_keys = ["<generated_key>"]
```

3. **Enable HTTPS** with reverse proxy (Nginx):
```nginx
server {
    listen 443 ssl http2;
    server_name api.supernovanetwork.xyz;
    
    ssl_certificate /etc/letsencrypt/live/api.supernovanetwork.xyz/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/api.supernovanetwork.xyz/privkey.pem;
    
    location / {
        proxy_pass http://127.0.0.1:8332;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # CORS headers (if needed in addition to backend CORS)
        add_header Access-Control-Allow-Origin "https://supernovanetwork.xyz" always;
        add_header Access-Control-Allow-Methods "POST, OPTIONS" always;
        add_header Access-Control-Allow-Headers "Content-Type, Authorization" always;
    }
    
    # WebSocket support (future)
    location /ws {
        proxy_pass http://127.0.0.1:8332;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

### Frontend Changes

Update `.env.production`:
```bash
NEXT_PUBLIC_API_URL=https://api.supernovanetwork.xyz
NEXT_PUBLIC_WS_URL=wss://api.supernovanetwork.xyz/ws
NEXT_PUBLIC_API_KEY=<secure_production_key>
```

---

## Monitoring & Analytics

### Backend Metrics

The node exposes Prometheus metrics on port 9000:
- `http://localhost:9000/metrics`

Key metrics to monitor:
- `supernova_block_height` - Current blockchain height
- `supernova_peer_count` - Number of connected peers
- `supernova_carbon_emissions` - Total carbon emissions
- `supernova_api_requests_total` - API request count
- `supernova_api_errors_total` - API error count

### Frontend Analytics

Consider adding:
- Plausible or PostHog for privacy-friendly analytics
- Error tracking with Sentry
- Performance monitoring with Vercel Analytics

---

## Security Checklist

### Backend
- [x] CORS properly configured
- [x] Bearer token authentication
- [x] Rate limiting enabled
- [x] Input validation on all parameters
- [ ] HTTPS enforced in production
- [ ] API key rotation policy
- [ ] Request logging for audit trail

### Frontend
- [ ] Environment variables not committed to git
- [ ] Sensitive data (API keys) stored securely
- [ ] Input sanitization on user-provided data
- [ ] Error messages don't expose sensitive info
- [ ] HTTPS enforced for all API calls in production
- [ ] CSP headers configured

---

## Next Steps

1. **Immediate** (Development):
   - Clone frontend repo
   - Update environment variables
   - Integrate API client
   - Test locally with running node

2. **Short-term** (Pre-launch):
   - Deploy backend to cloud infrastructure
   - Configure DNS and SSL
   - Update frontend with production URLs
   - Comprehensive integration testing

3. **Long-term** (Post-launch):
   - Implement WebSocket for real-time updates
   - Add transaction indexing for `gettransaction`
   - Build full block explorer
   - Add advanced charts and analytics

---

## Support & Resources

- **Backend Repo**: https://github.com/Carbon-Twelve-C12/supernova
- **Frontend Repo**: https://github.com/mjohnson518/supernova-web (private)
- **Website**: https://supernovanetwork.xyz/
- **API Docs**: http://localhost:8332/swagger-ui/ (when node running)
- **RPC Docs**: See `RPC_IMPLEMENTATION.md`

---

**Status**: Ready for integration! The Supernova testnet API is fully operational and awaiting frontend connection. 🚀
