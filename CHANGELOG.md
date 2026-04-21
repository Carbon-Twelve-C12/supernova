# Changelog

All notable changes to Supernova are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions follow [SemVer](https://semver.org/). Pre-release tags use the
`-RCN` suffix (Release Candidate *N*).

Supernova is a quantum-resistant, carbon-aware Proof-of-Work blockchain.
See [`docs/adr/`](docs/adr/) for the decisions behind the load-bearing
choices, and [`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md)
for the STRIDE analysis that guides hardening.

---

## [Unreleased]

Targeting **`v1.0.0-RC5`** (testnet launch) and **`v1.0.0`** (mainnet). This
section tracks work on `main` since `v1.0.0-RC4`.

### Added
- **Property-based tests** (`supernova-core/tests/proptests.rs`, 21
  strategies across difficulty adjustment, ML-DSA verify, hash
  determinism, transaction/block bincode round-trip, constant-time
  compare).
- **Nightly AFL++ fuzzing CI** (`.github/workflows/fuzz.yml`, five
  targets, 30-min runs, crash artifacts retained 30 days). Runbook in
  [`docs/testing/FUZZING.md`](docs/testing/FUZZING.md).
- **STRIDE threat model**
  ([`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md)) and
  [`docs/security/SECURITY_ASSUMPTIONS.md`](docs/security/SECURITY_ASSUMPTIONS.md).
- **Trivy filesystem / config / image scanning** wired into
  `.github/workflows/security-audit.yml` with SARIF upload to the
  GitHub Security tab; `.trivy.yaml` + `.trivyignore` baselines.
- **`subtle` workspace dependency**; every cryptographic byte compare
  in the HTLC / MAC / hash paths now uses
  `subtle::ConstantTimeEq::ct_eq`.
- **Per-address and per-IP faucet rate limiting** (24h window), reusing
  the PoW challenge from `node/src/network/peer_identity.rs`.
- **`s3-backup` feature flag** pulling in the AWS SDK for Lightning
  channel-state backup; backups are ML-DSA-signed before upload and
  verified on retrieval.
- **Lightning peer-backup P2P protocol** with request/response
  correlation and 30s timeout; new `BackupStore` / `BackupStoreAck` /
  `BackupRequest` / `BackupResponse` message variants.
- **`supernova-core/src/governance/treasury.rs`** with network-aware
  `treasury_address(Network)` returning an `m`-of-`n` SPHINCS+ multisig;
  coinbase-validation tests in
  `miner/tests/treasury_validation_tests.rs`.
- **Baseline metrics** in
  [`docs/BASELINE_RC4.md`](docs/BASELINE_RC4.md) (tests: 656 passed / 73
  failed, clippy panic-class violations: 289 in `supernova-core`).

### Changed
- **Panic-class clippy violations** (`unwrap_used`, `expect_used`,
  `panic`, `unreachable`, `todo`, `unimplemented`) remediated across the
  crypto, consensus, validation, network, and mempool paths; remaining
  work covers wallet, miner, storage, and RPC.
- `QuantumKeyPair` now derives `Zeroize, ZeroizeOnDrop`;
  `#[zeroize(skip)]` on `KeyRotationManager::previous_keys` removed.
- `static_assertions::assert_impl_all!(ApiFacade: Send, Sync)` re-enabled;
  `node/src/thread_safety_fix.rs` removed; the panicking construction
  fallback at `api_facade.rs:59` now propagates `NodeError`.

### Fixed
- Mempool fee-rate cap wiring (`285244c`).
- Argon2id-based keystore hashing (`fafc162`).
- Duplicate merge-residue files (`<name> 2.rs`) removed from tracked paths.

### Security
- See [`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md) for
  current STRIDE findings and open items. External third-party audit
  engagement is scheduled before the 1.0.0 mainnet tag.

---

## [1.0.0-RC4] — 2025-09-29

**Theme:** Consensus hardening and architectural consistency pass before the
mainnet-readiness audit.

### Added
- Weak subjectivity checkpoint enforcement
  (`supernova-core/src/consensus/weak_subjectivity.rs`).
- Time-warp attack prevention in difficulty adjustment.
- Fork resolution v2 (`supernova-core/src/consensus/fork_resolution_v2.rs`) —
  see [ADR-0005](docs/adr/0005-fork-resolution-v2.md).
- Eclipse-prevention anchor-connection policy and
  subnet/ASN/region diversity scoring.
- Lock-free UTXO cache backed by `DashMap` / `DashSet`.
- Chaos-testing scaffolding (`node/src/tests/chaos_testing.rs`).

### Changed
- Workspace crate renamed `btclib` → `supernova-core`.
- Network P2P message size cap reduced from 32 MB to 4 MB (DoS budget).
- `RC4 consistency` commit (`e025ce4`) aligned module exports, feature flags,
  and lints across all crates.

### Security
- P0 issues closed: consensus fork resolution race, UTXO double-spend, quantum
  signature algorithm-downgrade.
- P1 issues closed (7/7): HTLC quantum timeout, environmental oracle Byzantine
  consensus, mempool DoS, eclipse vectors, validation complexity, wallet HD
  predictability, storage corruption recovery.

---

## [1.0.0-RC3] — 2025-06-01

**Theme:** First full external-audit-grade drop with testnet deployment
infrastructure.

### Added
- Production deployment infrastructure: `docker/`, `helm/supernova/`,
  `kubernetes/` manifests.
- Prometheus + Grafana + AlertManager observability stack.
- Testnet configuration (`testnet/`, `config/testnet.example.toml`).
- Genesis coordination flow for multi-operator launches.

### Security
- Broad security hardening across cryptographic, consensus, and networking
  subsystems.
- API rate limiting (`node/src/api/rate_limiter.rs`).
- Transaction fee integer-overflow protections.

---

## [1.0.0-RC2] — 2025-05-27

**Theme:** Lightning Network completion with quantum-safe HTLCs.

### Added
- Lightning Network implementation completed: channel manager, watchtower,
  multi-path payments, channel backup.
- Quantum-security layer for Lightning messages
  (`supernova-core/src/lightning/quantum_security.rs`).
- Key rotation mechanism and quantum canary system.

### Changed
- Compilation errors resolved workspace-wide; documentation refreshed.

---

## [1.0.0-RC1] — 2025-05-25

**Theme:** Lightning Network complete with quantum security.

### Added
- First RC tag of the 1.0.0 series. Lightning channels, HTLCs, gossip, and
  routing on a quantum-secure substrate.

---

## [0.9.9-FRC] — 2025-05-06

**Theme:** Feature-complete candidate — all 1.0.0-scope subsystems landed.

### Added
- Full post-quantum cryptographic stack (ML-DSA, SPHINCS+, ML-KEM, SHA3-512).
- UTXO model with parallel validation.
- P2P networking with gossipsub.
- HD wallet with PQ derivation paths.
- Mining subsystem with difficulty adjustment.
- Environmental oracle with Byzantine-fault-tolerant aggregation.

---

## [0.1.0 – 0.9.x] — 2024-11-28 → 2025-05-05

**Theme:** Initial implementation — workspace scaffolding through feature
completion of individual subsystems.

### Added
- Workspace structure and core types (Block, BlockHeader, Transaction).
- Merkle tree, mempool, P2P networking, mining, storage layer.
- First wallet implementation and CLI.
- Phase 1 cryptographic enhancements including hybrid classical+PQ signatures.

Historical detail lives in the commit log; `git log --since=2024-11-28 --until=2025-05-05`
is authoritative for this period.

---

## Conventions

- **RCN** suffix marks a Release Candidate; promote to the next number only
  after all P0/P1 items in the previous RC are resolved.
- Entries are grouped **Added / Changed / Fixed / Security / Removed** per
  Keep-a-Changelog.
- File-path references use the repository-relative form; line numbers are
  omitted because they drift — grep the symbol instead.

---

## Release checklist (for RC tags and 1.0.0)

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-features -- -D warnings` — must pass.
3. `cargo test --workspace --release` — must pass.
4. `cargo audit` — no open advisories.
5. `cargo deny check` — no license, advisory, or duplicate-version violations.
6. CHANGELOG `Unreleased` section promoted to a dated version heading.
7. Tag signed with the release GPG key; artifacts signed with cosign (see
   `.github/workflows/release.yml`).
8. External audit report filed for the RC under review (mainnet only).
