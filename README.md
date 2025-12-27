# Supernova

**Quantum-Resistant, Carbon-Negative Proof-of-Work Blockchain**

<div align="center">
  <a href="https://supernovanetwork.xyz/"><img src="https://img.shields.io/badge/website-supernovanetwork.xyz-blue" alt="Website" /></a>
  <a href="https://github.com/Carbon-Twelve-C12/supernova/releases"><img src="https://img.shields.io/badge/version-1.0.0--RC4-green" alt="Version" /></a>
  <a href="https://deepwiki.com/Carbon-Twelve-C12/supernova"><img src="https://img.shields.io/badge/Ask%20DeepWiki-222222?logo=deepwiki" alt="DeepWiki"></a>
  <br/>
  <strong>Status: Testnet Ready</strong>
</div>

---

Supernova is a PoW blockchain written in Rust, built to address three critical challenges: quantum computing threats, environmental sustainability, and scalable transactions. The protocol implements NIST-standardized post-quantum cryptography throughout the entire stack.

## Key Features

### Quantum Resistance
- **ML-DSA (Dilithium)** for transaction signatures
- **SPHINCS+** for stateless wallet recovery
- **ML-KEM (Kyber)** for P2P key exchange
- **SHA3-512** hashing throughout

### Carbon-Negative Mining
- Real-time emissions tracking on-chain
- Block reward bonuses for verified renewable energy
- Automated environmental treasury allocation

### Lightning Network
- Quantum-secure channel operations
- SHA3-512 HTLCs
- Post-quantum onion routing

## Quick Start

```bash
# Clone and build
git clone https://github.com/Carbon-Twelve-C12/supernova.git
cd supernova
cargo build --release

# Run testnet node
./target/release/supernova-node --network testnet
```

## Run a Node

Want to join the network? Running your own node helps decentralize Supernova.

**[Read the Node Operator Guide](docs/RUNNING_A_NODE.md)** for complete setup instructions.

### Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 4 cores | 8+ cores |
| RAM | 8 GB | 16-32 GB |
| Storage | 100 GB SSD | 200+ GB NVMe |

## Architecture

```
supernova/
├── supernova-core/     # Core blockchain logic, consensus, cryptography
├── node/               # Full node implementation, P2P, RPC
├── wallet/             # Quantum-resistant wallet
└── btclib/             # Bitcoin-compatible primitives
```

## Status

**Current Release:** v1.0.0-RC4 (Release Candidate 4)

| Component | Status |
|-----------|--------|
| Core Protocol | Testnet Ready |
| Quantum Cryptography | Complete |
| Lightning Network | Complete |
| P2P Networking | Complete |
| Security Hardening | Complete |
| Test Coverage | 98%+ |

## Security

Supernova implements defense-in-depth with:
- Post-quantum cryptography at every layer
- Algorithm downgrade prevention
- All security tests 100% passing

See [SECURITY.md](SECURITY.md) for responsible disclosure.

## Documentation

- [Node Operator Guide](docs/RUNNING_A_NODE.md)
- [Architecture Overview](docs/supernova_overview.md)
- [API Reference](docs/api/README.md)
- [Contributing Guide](CONTRIBUTING.md)

## License

MIT License - see [LICENSE](LICENSE) for details.
