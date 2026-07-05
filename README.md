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
- **ML-DSA (Dilithium)** for transaction signatures and wallet keys
- **SPHINCS+** stateless-signature support for wallet recovery
- **ML-KEM (Kyber)** key-encapsulation primitive implemented 
- **SHA3-512** hashing throughout

### Carbon-Negative Mining
- On-chain emissions calculation and energy-source classification
- Block reward bonuses for renewable energy
- Environmental treasury accounting logic

### Lightning Network (In Progress)
- Quantum-secure channel state machine
- SHA3-512 HTLC data structures
- Post-quantum onion routing (planned)

## Quick Start

```bash
# Clone and build
git clone https://github.com/Carbon-Twelve-C12/supernova.git
cd supernova
cargo build --release

# Run testnet node (auto-loads ./config/node.toml from the repo root)
./target/release/supernova-node
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
├── supernova-core/      # Core types, consensus, crypto, lightning
├── node/                # Full node implementation, P2P, RPC, mempool
├── miner/               # Mining implementation, difficulty adjustment
├── wallet/              # Quantum-resistant HD wallet
├── quantum_validation/  # ML-DSA / SPHINCS+ signature verification
└── cli/                 # Command-line tools
```

## Status

**Current Release:** v1.0.0-RC4 (Release Candidate 4)

| Component | Status |
|-----------|--------|
| Core Protocol | Testnet Ready |
| Quantum Cryptography | ML-DSA complete; ML-KEM primitive implemented but not yet wired into P2P; SPHINCS+/Hybrid signing planned; bulletproof range-proof verifier fail-closed (see ADR-0008) |
| Lightning Network | In Progress (architectural foundation) |
| P2P Networking | In Progress (ML-KEM key exchange not yet wired into transport) |
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
