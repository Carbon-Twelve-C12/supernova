# Supernova Testnet Guide

Welcome to the Supernova Carbon-Negative Quantum-Secure Blockchain Testnet! 🌟

## Quick Start for Testnet Participants

### 1. Get Test NOVA Tokens

Visit our faucet to receive free testnet tokens:
- **Faucet URL**: https://faucet.testnet.supernovanetwork.xyz
- You'll receive 1,000 test NOVA tokens
- One request per hour per address

### 2. Connect to the Network

#### Option A: Run a Full Node (Recommended)

```bash
# Using Docker
docker run -d \
  --name supernova-node \
  -p 8333:8333 \
  -p 8332:8332 \
  -v supernova-data:/data/supernova \
  mjohnson518/supernova-node:testnet \
  --connect=testnet.supernovanetwork.xyz:8333 \
  --network=testnet
```

#### Option B: Light Client

Download our desktop wallet (coming soon) or use the web wallet at:
- **Web Wallet**: https://wallet.testnet.supernovanetwork.xyz

### 3. View Network Statistics

- **Dashboard**: https://testnet.supernovanetwork.xyz
- **API Documentation**: https://api.testnet.supernovanetwork.xyz/docs
- **Network Metrics**: https://grafana.testnet.supernovanetwork.xyz

## Features to Test

### 🔐 Quantum-Secure Transactions
- All transactions use CRYSTALS-Dilithium signatures
- Test quantum-resistant features in your wallet
- Verify signatures with our quantum validation tools

### 🌍 Environmental Features
- **Carbon Tracking**: View real-time carbon footprint
- **Green Mining**: Earn bonus rewards with renewable energy
- **Carbon Credits**: Test carbon offset transactions

### ⚡ Lightning Network
- Open quantum-secure payment channels
- Test instant green payments
- Route payments through eco-friendly nodes

## For Developers

### API Endpoints

```javascript
// Connect to testnet API
const API_BASE = 'https://api.testnet.supernovanetwork.xyz';

// Example: Get blockchain info
fetch(`${API_BASE}/v1/blockchain/info`)
  .then(res => res.json())
  .then(data => console.log(data));

// Example: Get carbon metrics
fetch(`${API_BASE}/v1/environmental/carbon`)
  .then(res => res.json())
  .then(data => console.log(data));
```

### Running Your Own Node

#### System Requirements
- 2 CPU cores
- 4GB RAM
- 50GB SSD storage
- Ubuntu 20.04+ or similar

#### Installation Script

```bash
# Quick install script
curl -sSL https://testnet.supernovanetwork.xyz/install.sh | bash
```

#### Manual Installation

```bash
# 1. Clone repository
git clone https://github.com/mjohnson518/supernova.git
cd supernova

# 2. Build from source
cargo build --release --bin supernova-node

# 3. Run with testnet configuration
./target/release/supernova-node \
  --network testnet \
  --data-dir ~/.supernova/testnet \
  --connect testnet.supernovanetwork.xyz:8333
```

### Configuration Options

Create `~/.supernova/testnet/config.toml`:

```toml
[network]
network = "testnet"
bootstrap_nodes = [
    "testnet.supernovanetwork.xyz:8333",
    "testnet.supernovanetwork.xyz:8343"
]

[node]
enable_mining = true
enable_lightning = true
quantum_signatures = true

[environmental]
track_carbon = true
report_renewable = true
oracle_endpoints = [
    "https://testnet-oracle1.supernovanetwork.xyz",
    "https://testnet-oracle2.supernovanetwork.xyz"
]

[rpc]
enable = true
bind = "127.0.0.1:8332"
```

## Testing Scenarios

### 1. Green Mining Test
```bash
# Set your renewable energy percentage
supernova-cli set-renewable-percentage 75

# Start mining with green bonus
supernova-cli mine --threads 2
```

### 2. Quantum Signature Test
```bash
# Create quantum-secure transaction
supernova-cli send \
  --to supernova1abc... \
  --amount 10 \
  --quantum-scheme dilithium3
```

### 3. Lightning Channel Test
```bash
# Open a green Lightning channel
supernova-cli lightning open-channel \
  --peer 03abc...@testnet.supernovanetwork.xyz:9735 \
  --amount 0.1 \
  --green-routing true
```

### 4. Carbon Credit Test
```bash
# Purchase carbon credits
supernova-cli buy-carbon-credits \
  --amount 1000 \
  --provider verified-offsets
```

## Troubleshooting

### Connection Issues
```bash
# Check node status
supernova-cli getinfo

# View peer connections
supernova-cli getpeers

# Check sync status
supernova-cli getsyncstatus
```

### Common Issues

1. **"Connection refused"**: Ensure firewall allows ports 8333, 8332
2. **"Sync stuck"**: Try connecting to different bootstrap node
3. **"Invalid signature"**: Update to latest version with quantum support

## Community

- **GitHub**: https://github.com/mjohnson518/supernova


## Bug Reporting

Found an issue? Please report it:
1. **GitHub Issues**: https://github.com/mjohnson518/supernova/issues
2. **Bug Bounty Program**: security@supernovanetwork.xyz

### Rewards for Finding:
- Critical bugs: 1,000 - 10,000 test NOVA
- Medium bugs: 100 - 1,000 test NOVA
- Minor bugs: 10 - 100 test NOVA

## Advanced Features

### Environmental Oracle Integration
```bash
# Register as environmental data oracle
supernova-cli oracle register \
  --type environmental \
  --stake 10000 \
  --region north_america
```

### Custom Smart Contracts
```rust
// Example: Carbon-neutral smart contract
#[quantum_secure]
#[carbon_neutral]
contract GreenDAO {
    fn vote(&mut self, proposal: u64) -> Result<()> {
        // Voting requires carbon offset
        require_carbon_neutral!(msg.sender);
        // Vote logic here
    }
}
```

## Network Parameters

| Parameter | Value |
|-----------|-------|
| Block Time | 10 seconds |
| Block Reward | 50 NOVA + green bonus |
| Quantum Security | NIST Level 3 |
| Carbon Tracking | Real-time |
| Lightning Capacity | Unlimited |
| Testnet Reset | Never (persistent) |

## Security Notes

- This is a **TESTNET** - tokens have no real value
- Test wallets should not be used for mainnet
- Report any security issues responsibly
- Quantum signatures are experimental

## Upcoming Features

- [ ] Mobile wallet app
- [ ] DEX integration
- [ ] Carbon credit marketplace
- [ ] Advanced quantum algorithms
- [ ] Cross-chain bridges

---

**Ready to build the future of sustainable blockchain?** 🚀

Join us in creating the world's first carbon-negative, quantum-secure blockchain! 