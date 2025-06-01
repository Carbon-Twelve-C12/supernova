# Supernova Testnet Node Operator Quick Start

## 🚀 5-Minute Setup

### Option 1: Docker (Recommended)
```bash
# Pull and run the node
docker run -d \
  --name supernova-node \
  -p 8333:8333 \
  -p 8332:8332 \
  -v supernova-data:/data/supernova \
  mjohnson518/supernova-node:testnet \
  --connect=testnet.supernovanetwork.xyz:8333 \
  --network=testnet
```

### Option 2: One-Line Script
```bash
curl -sSL https://raw.githubusercontent.com/mjohnson518/supernova/main/deployment/scripts/node-setup.sh | bash
```

## 📋 Pre-flight Checklist

- [ ] **System Requirements Met**
  - 2+ CPU cores
  - 4GB+ RAM  
  - 50GB+ SSD storage
  - Ubuntu 20.04+ or Docker

- [ ] **Ports Open**
  - 8333 (P2P)
  - 8332 (RPC)
  - 9735 (Lightning, optional)

- [ ] **Wallet Address Ready**
  - Get from web wallet: https://wallet.testnet.supernovanetwork.xyz
  - Or generate locally: `supernova-cli getnewaddress`

## 🎯 First Steps After Setup

1. **Get Test Tokens**
   ```bash
   curl -X POST https://faucet.testnet.supernovanetwork.xyz/api/v1/faucet \
     -H "Content-Type: application/json" \
     -d '{"address": "YOUR_TESTNET_ADDRESS"}'
   ```

2. **Check Node Status**
   ```bash
   docker exec supernova-node supernova-cli getinfo
   ```

3. **Enable Green Mining** (Optional)
   ```bash
   docker exec supernova-node supernova-cli set-renewable-percentage 75
   ```

4. **Open Lightning Channel** (Optional)
   ```bash
   docker exec supernova-node supernova-cli lightning open-channel \
     --peer 03abc...@testnet.supernovanetwork.xyz:9735 \
     --amount 0.1
   ```

## 🔍 Monitoring Your Node

- **Local Stats**: http://localhost:8332/stats
- **Network Dashboard**: https://testnet.supernovanetwork.xyz
- **Your Node on Explorer**: https://explorer.testnet.supernovanetwork.xyz/node/YOUR_NODE_ID

## 🛠️ Useful Commands

```bash
# View logs
docker logs -f supernova-node

# Check sync status
docker exec supernova-node supernova-cli getsyncstatus

# View peer connections
docker exec supernova-node supernova-cli getpeers

# Check environmental status
docker exec supernova-node supernova-cli environmental status
```

## 💰 Earning Test Rewards

1. **Mining**: Earn block rewards + green bonuses
2. **Lightning Routing**: Earn fees from payment routing
3. **Environmental Oracles**: Provide carbon data
4. **Bug Bounties**: Find and report issues

## 📚 Need More Help?

- **Full Guide**: [TESTNET_GUIDE.md](TESTNET_GUIDE.md)
- **Troubleshooting**: [TROUBLESHOOTING.md](TROUBLESHOOTING.md)
- **Discord**: https://discord.gg/supernova
- **API Docs**: https://api.testnet.supernovanetwork.xyz/docs

---

**Welcome to the future of sustainable blockchain!** 🌍⚡🔐 