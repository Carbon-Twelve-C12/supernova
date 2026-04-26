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
- **`replace_all`'d `to_ascii_lowercase()` normalization** to the API-key
  placeholder check (`node/src/api/server.rs`); marker list expanded to
  `change-me`, `changeme`, `replace-me`, `replaceme`, `example`,
  `default`, `test`, `secret`, `demo` — case-swapped readme paste-ins
  no longer slip through.
- **Surrounding-whitespace rejection** for API keys: `validate_api_keys`
  now refuses keys whose raw form differs from the trimmed form, so
  TOML-padded values fail loud at startup instead of silently 401-ing
  every client request that uses the trimmed (correct) Bearer token.
- **Atomic-swap refund-flow infrastructure** (Phase 3 C4):
  - `SupernovaHTLC::build_refund_transaction(funding_txid, funding_vout)`
    constructs an unsigned consensus-correct refund tx with state-machine
    guards (rejects `Claimed`/`Refunded` HTLCs and dust-amount refunds)
    and `sequence = relative_timeout` for sequence-based locktime
    defense-in-depth.
  - `SwapSession::funding_outpoint: Option<FundingOutpoint>` field with
    `set_funding_outpoint()` setter and `refund_funding_outpoint()`
    resolver that falls back to a deterministic synthetic outpoint if
    the real on-chain reference is not yet recorded. `#[serde(default)]`
    keeps RC4-era persisted sessions deserialisable.
  - `RefundSigner` async trait + typed `RefundSignerError` for producing
    MLDSA signatures over the canonical refund message.
  - `RefundBroadcaster` async trait + typed `RefundBroadcastError`
    distinguishing mempool rejection from p2p-stack failures.
  - `AtomicSwapRpcImpl::with_refund_signer` / `with_refund_broadcaster`
    builder methods for opt-in production wiring without breaking the
    existing 8+ test callers of `new()`.
  - `supernova-core/examples/refund_flow_demo.rs` — end-to-end working
    example with mock signer/broadcaster, registered as
    `[[example]]` and runnable via
    `cargo run --example refund_flow_demo --features atomic-swap`.
- **Phase 5 baseline performance measurements** captured in
  [`docs/performance/BASELINE_MEASUREMENTS.md`](docs/performance/BASELINE_MEASUREMENTS.md)
  (M1 Pro / 10 cores / Rust 1.90.0):
  - Block propagation: 944 K header-hashes/s, 1.03 M PoW-checks/s,
    45.8 K tx-merkle-verify/s @ 1000 txs, 1.15 GiB/s block serialise,
    903 MiB/s deserialise.
  - UTXO cache: 5.07 M cached-lookups/s @ 10 K UTXOs (drops to 1.34 M @
    500 K), ~220 ns flat miss-lookup latency, 558 K UTXOs/s flushed
    @ 10 K batch.
  - Signature verify: **176 K** ML-DSA-3 parallel sigs/s on 10 cores
    (10 K sigs in 56.8 ms — ~50× margin against the plan target of
    "10 K sigs in <3 s"); 23 K sequential single-core. ML-DSA-2 / -3 /
    -5 single-sig latencies: 28.9 µs / 43.2 µs / 69.2 µs.
  - Notable finding flagged in-doc: `utxo_spend/single_spend` measures
    1.55 ns, implausibly fast for a real DB write — likely measuring an
    in-memory bitset flip rather than the durable write path. Harness
    refurbishment pending.

### Changed
- **Panic-class clippy violations** (`unwrap_used`, `expect_used`,
  `panic`, `unreachable`, `todo`, `unimplemented`) remediated **workspace-
  wide**. `cargo clippy --workspace --all-features` now produces zero
  deny-class errors; the `#![cfg_attr(not(test), deny(...))]` policy in
  `supernova-core/src/lib.rs` and `node/src/lib.rs` is genuinely
  enforced. Closed-out paths include lightning, environmental oracle,
  manual-verification, monitoring (quantum-signature benchmarks),
  environmental verification, security-mitigation, security/quantum-
  canary, environmental/carbon-tracking, testnet/regression-testing,
  security/quantum-security-audit, environmental/renewable-validation,
  wallet/quantum-wallet, types/{block,transaction,extended_transaction,
  transaction_dependency}, atomic-swap (monitor / api / cache / htlc /
  bitcoin-adapter), testnet/{test-harness,config,faucet}, util/merkle,
  storage/utxo-set, environmental/emissions,
  environmental/oracle-registry, lightning/wallet, mining, plus
  workspace-root `api.rs`. Lock-poisoning is now propagated as typed
  errors on `Result`-returning paths (`*Error::LockPoisoned` variants
  added per crate) and recovered via `unwrap_or_else(|p| p.into_inner())`
  on infallible read-only accessors.
- **`Transaction::hash()`** and `ConfidentialTransaction::hash()`
  consensus-load-bearing serialization: `bincode::serialize(...).unwrap()`
  → `unwrap_or_else(|e| { error!(...); Vec::new() })` with explicit
  doc on why the failure arm is unreachable for standard `Serialize`
  derives. Cascading `Result<[u8; 32], _>` was infeasible (called
  pervasively); the empty-vec fallback yields the known SHA-256-of-empty
  constant `e3b0c4429…` which is recognisable on inspection if it ever
  appears.
- **`Block::serialize()` / `Block::genesis()` / `Block::new_with_params`**
  panic paths replaced with `tracing::error!` + safe fallback;
  consensus validators downstream still reject any block built with the
  fallback values.
- **`BulletproofRangeProof::verify`** fails closed on `#[cfg(not(test))]`
  builds. The previous structural-validation-then-`return true` path
  certified hand-constructed forgeries against `ConfidentialTransaction::
  verify_range_proofs`, which reached `transaction_processor.rs:178` —
  i.e. a confidential-amount-forgery vector on the consensus path.
  Tests retain the positive-case path via `#[cfg(test)]`. Docstrings on
  both `verify` and `verify_range_proof` rewritten to describe the stub
  honestly.
- **API server config wiring**: `ApiServer::new` and `create_api_server`
  now take an `ApiConfig` directly. Previously `main.rs` constructed the
  server with literal `"0.0.0.0"` plus port and let `ApiServer` store
  `ApiConfig::default()`, dropping the operator's `[api]` TOML on the
  floor; with the post-RC4 fail-closed default (`api_keys = None,
  enable_auth = true`) every upgraded deployment silently lost its API
  on first boot.
- **`build_cors`** in `node/src/api/server.rs` now calls
  `allow_any_origin().send_wildcard()` (not just `send_wildcard()`) for
  `cors_allowed_origins = ["*"]`. Without `allow_any_origin()`, every
  cross-origin request was rejected with `OriginNotAllowed` despite the
  warning log claiming "ANY origin" was allowed.
- **`AuthRateLimiter` is now shared across actix-web workers**. Each
  per-worker `HttpServer::new` factory call previously allocated a fresh
  `failed_attempts` map, so the brute-force ceiling was N × 5 attempts
  per 5-minute window (typical N = 8–32). One `Arc<AuthRateLimiter>` is
  now built outside the closure and threaded into every worker via new
  `from_validated_keys_with_rate_limiter` / `disabled_with_rate_limiter`
  constructors.
- **`atomic_swap::metrics::metrics()`** auto-initializes the
  `OnceLock<AtomicSwapMetrics>` on first access. Previously every
  recording helper short-circuited until something explicitly called
  `init_metrics()` — and nothing did, so every counter silently stayed
  at zero in any build that enabled the `atomic-swap` feature.
- **`deny.toml`** modernized to cargo-deny v0.16+ schema (removed
  `vulnerability`, `unmaintained` (top-level), `unlicensed`, `copyleft`,
  `allow-osi-fsf-free`, `notice`; added `version = 2` to `[advisories]`
  and `[licenses]`; added `[graph].all-features = true` so feature-gated
  deps like `aws-sdk-s3` get audited; added `Unicode-3.0` and `OpenSSL`
  to the license allowlist).
- `QuantumKeyPair` now derives `Zeroize, ZeroizeOnDrop`;
  `#[zeroize(skip)]` on `KeyRotationManager::previous_keys` removed.
- `static_assertions::assert_impl_all!(ApiFacade: Send, Sync)` re-enabled;
  `node/src/thread_safety_fix.rs` removed; the panicking construction
  fallback at `api_facade.rs:59` now propagates `NodeError`.
- **`AtomicSwapRpcImpl::refund_swap`** RPC: returns a real
  `Transaction::hash()`-derived txid instead of the previous
  `STUB_refund_<id>` sentinel string. When a `RefundSigner` is wired,
  the signature is embedded in the input's `signature_script` (which
  changes the txid — the *signed* txid is the one returned). When a
  `RefundBroadcaster` is also wired, the signed tx is submitted via the
  trait. Audit log line now reports `(signed=<bool>, broadcast=<bool>)`
  so operators can see exactly how far down the pipeline a refund got.
- **`SupernovaHTLC::create_refund_message`** visibility raised from
  `pub(crate)` to `pub` so external `RefundSigner` implementations can
  produce a signature over the same canonical bytes that `verify_refund`
  later checks.

### Fixed
- Mempool fee-rate cap wiring (`285244c`).
- Argon2id-based keystore hashing (`fafc162`).
- Duplicate merge-residue files (`<name> 2.rs`) removed from tracked
  paths; untracked `Cargo 2.toml` / `.clippy.toml.toml` deleted from
  the working tree.
- `block_validation_tests::create_test_block` now constructs a coinbase
  with a proper 5% regtest treasury output so the
  `validate_coinbase_treasury` rule (added with
  `governance/treasury.rs`) doesn't reject the test fixture. Restores
  the 2 `block_validation_tests` failures to passing.
- nginx preflight blocks emitted `Content-Type: text/plain charset=UTF-8`
  (missing semicolon per RFC 9110 §8.3.1); fixed to
  `text/plain; charset=UTF-8` in both
  `nginx_main_testnet.conf` and `nginx_testnet_config.conf`.
- `wallet/src/quantum_wallet/address.rs::Address::to_string` inherent
  method removed — it shadowed the `Display::fmt` impl
  (`clippy::should_implement_trait`); both produced identical strings,
  callers now route through `ToString::to_string()` driven by `Display`.
- Surrounding-whitespace rejection on API keys (see Added).
- **Bench harnesses are buildable for the first time post-rename**:
  - `supernova-core/benches/comprehensive_benchmarks.rs` deleted (430
    lines of stale `btclib::*` references — predated the workspace
    rename; types like `FalconKeyPair`, `KyberKEM`, `TxInput/TxOutput`
    no longer exist). `tps_harness.rs` already covers the same surface
    against current types.
  - `node/benches/utxo_benchmarks.rs:243` borrow-checker error fixed —
    `let utxo = create_test_utxo(&mut rng, rng.gen())` had overlapping
    `&mut rng` borrows; split into two statements.
  - `cargo build --benches -p supernova-core` and `-p supernova-node`
    both green; numbers captured in §2.4 / §2.5 of the baseline doc.
- **Cargo.toml examples-directory documentation**: added a comment
  block annotating that `examples/{atomic_swap_demo, lightning_demo,
  environmental_demo, ...}` reference field shapes that predate the
  post-RC4 refactor (e.g. `ChannelConfig.max_htlc_value_in_flight_msat`
  no longer exists) and are unregistered intentionally — historical
  documentation rather than tested examples. The new
  `refund_flow_demo` and the existing `memory_profile` are the only
  registered examples.

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
