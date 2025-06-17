# Supernova Testnet Connection Guide

## Testnet Endpoints

The Supernova testnet is accessible via the following public endpoints:

### Web Services
- **Explorer**: https://explorer.testnet.supernovanetwork.xyz
- **Network Status**: https://status.testnet.supernovanetwork.xyz
- **Faucet**: https://faucet.testnet.supernovanetwork.xyz
- **API Documentation**: https://api.testnet.supernovanetwork.xyz

### API Endpoints
Base URL: `https://api.testnet.supernovanetwork.xyz`

Example endpoints:
- `/blockchain/info` - Get blockchain information
- `/node/status` - Get node status
- `/network/peers` - List connected peers

### Connecting Your Node

To connect your node to the testnet:

1. Set the testnet flag when starting your node:
   ```bash
   supernova-node --testnet
   ```

2. The node will automatically discover and connect to testnet peers.

3. For manual peer connection, use:
   ```bash
   supernova-node --testnet --bootnodes "/dns/testnet.supernovanetwork.xyz/tcp/30333"
   ```

### Environment Variables

For development scripts, set these environment variables:
```bash
export TESTNET_API_URL="https://api.testnet.supernovanetwork.xyz"
export TESTNET_EXPLORER_URL="https://explorer.testnet.supernovanetwork.xyz"
```

## Getting Testnet Tokens

Visit the faucet at https://faucet.testnet.supernovanetwork.xyz to request testnet tokens.

## Network Parameters

- Network ID: `supernova-testnet`
- Chain ID: `1337`
- Block Time: ~10 seconds
- Consensus: Proof of Work (quantum-resistant) 