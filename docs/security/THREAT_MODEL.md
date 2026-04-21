# Supernova Threat Model

**Version:** 1.0.0-RC4 → 1.0.0
**Methodology:** STRIDE per subsystem, with explicit trust boundaries and
cross-cutting concerns.
**Audience:** External security auditors (Phase 6 procurement), node
operators, protocol contributors.
**Companion:** [`SECURITY_ASSUMPTIONS.md`](./SECURITY_ASSUMPTIONS.md).

STRIDE categories:

| Code | Category                | Violated property |
| ---- | ----------------------- | ----------------- |
| S    | Spoofing                | Authentication    |
| T    | Tampering               | Integrity         |
| R    | Repudiation             | Non-repudiation   |
| I    | Information disclosure  | Confidentiality   |
| D    | Denial of service       | Availability      |
| E    | Elevation of privilege  | Authorization     |

Throughout this document, "mitigated" means the stated control is implemented
in the current tree at a cited path. "Residual" means the risk is accepted
under the listed assumption. "Open" means the risk is tracked and not yet
fully closed; every open item is cross-referenced to the production-readiness
plan.

---

## 1. Trust boundaries and system context

```
  ┌──────────────────┐   ┌──────────────────┐   ┌─────────────────┐
  │  End users       │   │  Node operators   │   │  External audit │
  │  (wallets, CEX)  │   │  (miners, relays) │   │  & oracles      │
  └────────┬─────────┘   └─────────┬─────────┘   └────────┬────────┘
           │ RPC / HTTPS           │ Local shell /        │ Signed feeds
           │ JSON-RPC              │ systemd               │ (env oracle)
           ▼                       ▼                       ▼
  ┌──────────────────────────────────────────────────────────────────┐
  │                      Supernova Node Process                       │
  │                                                                   │
  │   ┌──────────────┐    ┌──────────────┐    ┌─────────────────┐     │
  │   │ RPC/API      │◀──▶│ Consensus    │◀──▶│ Mempool         │     │
  │   │ (JSON-RPC,   │    │ (fork choice,│    │ (fee / DoS)     │     │
  │   │  utoipa)     │    │  difficulty) │    └────────┬────────┘     │
  │   └──────┬───────┘    └──────┬───────┘             │              │
  │          │                   │                     ▼              │
  │          │            ┌──────┴──────────────────────────────┐     │
  │          │            │  Block / Tx / Script validation      │     │
  │          │            └──────┬──────────────────────────────┘     │
  │          │                   ▼                                     │
  │          │            ┌──────────────┐    ┌─────────────────┐     │
  │          │            │ Storage      │    │ Wallet manager   │     │
  │          │            │ (sled + UTXO)│    │ (HD, keystore)   │     │
  │          │            └──────────────┘    └─────────────────┘     │
  │          │                                                         │
  │          │            ┌──────────────┐    ┌─────────────────┐     │
  │          └───────────▶│ Lightning    │◀──▶│ P2P / libp2p     │     │
  │                       │ (channels,   │    │ (ML-KEM, noise   │     │
  │                       │  HTLCs)      │    │  handshake)      │     │
  │                       └──────────────┘    └─────────────────┘     │
  └──────────────────────────────────────────────────────────────────┘
           │                       │                       │
           │ Peer messages         │ Local files            │ Remote backup
           ▼                       ▼                       ▼
  ┌──────────────────┐   ┌──────────────────┐   ┌─────────────────┐
  │  Network peers   │   │  Disk (sled,     │   │  S3 / peer      │
  │  (honest +       │   │  keystore.enc)    │   │  backup targets │
  │  Byzantine)      │   │                   │   │                 │
  └──────────────────┘   └──────────────────┘   └─────────────────┘
```

Trust boundaries (TB):

- **TB1 — Internet ↔ P2P layer.** Peers are assumed adversarial until they
  pass handshake and proof-of-work/proof-of-identity checks.
- **TB2 — RPC ↔ node core.** RPC callers are untrusted until authenticated;
  admin-scoped methods require JWT with the `admin` claim.
- **TB3 — Node ↔ disk.** Filesystem is trusted for availability, untrusted
  for confidentiality unless the keystore is encrypted (Argon2id + AEAD).
- **TB4 — Node ↔ remote backup.** S3 / peer-backup targets are untrusted;
  envelopes are signed (ML-DSA) and AEAD-encrypted before upload.
- **TB5 — Wallet ↔ signing oracle.** The signing key never leaves the wallet
  process; RPC exposes only signed artifacts.
- **TB6 — Node ↔ environmental oracle.** Oracle feeds are Byzantine-tolerant
  via 2f+1 aggregation; a single oracle cannot move policy inputs.

---

## 2. Global assumptions

1. **Adversary model — §1.1:** computationally bounded classical adversary
   and a future CRQC (cryptographically relevant quantum computer). All
   long-lived secrets must remain secure under Shor/Grover.
2. **Honest majority (PoW):** ≥ 50 % of network hashrate is honest. Weak
   subjectivity checkpoints additionally defeat long-range rewrites.
3. **Clock skew:** Node clocks are within ± 2 hours of median peer time;
   timestamps beyond this window are rejected (see §5.3).
4. **libp2p Noise transport** provides authenticated encryption for P2P
   traffic post-handshake; the handshake itself uses ML-KEM-768 for shared
   secret establishment.
5. **RNG:** `OsRng` (system CSPRNG) is unpredictable. The wallet also mixes
   with `getrandom` via `rand_core`.
6. **Upstream cryptography:** `pqcrypto-dilithium`, `pqcrypto-sphincsplus`,
   `pqcrypto-kyber`, `sha3`, and `blake3` are implemented correctly; their
   constant-time claims are trusted. External audit should re-verify.

Full list in [`SECURITY_ASSUMPTIONS.md`](./SECURITY_ASSUMPTIONS.md).

---

## 3. Cryptographic primitives

**Surface:** `supernova-core/src/crypto/{quantum,signature,kem,hybrid,
key_rotation,hash}.rs`, `quantum_validation/src/*`.

### 3.1 STRIDE

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| Signature forgery via algorithm downgrade | S / E | Adversary claims hybrid signature valid with only classical half | Hybrid verifier requires **both** ECDSA and ML-DSA paths to pass (`signature.rs`); downgrade bit is authenticated as part of the TLV structure |
| ML-DSA / SPHINCS+ key recovery by quantum adversary | S | CRQC breaks classical signatures | All long-lived keys are NIST PQC Level ≥ 2; coinbase, treasury, and Lightning use SPHINCS+ (stateless, conservative security) for additional defence in depth |
| Key re-use across domains | T | Same private key signs multiple payload types | Domain separation via prefix bytes in all hash-to-curve / hash-to-signature flows (`hash.rs`) |
| Timing side-channel in preimage / HMAC compare | I | Variable-time `==` over secret bytes | Constant-time compare via `subtle::ConstantTimeEq` in `lightning/{channel,payment,onion,quantum_channel,wallet,atomic_operations}.rs` (Phase 3 C1) |
| Secret-key exfiltration via memory disclosure | I | Process dump / core file contains key bytes | `Zeroize, ZeroizeOnDrop` on `QuantumSecretKey`, `KemKeyPair`, `QuantumSigningKey`, `KeyRotationManager`. `QuantumKeyPair` wrapper remediation tracked under Phase 1 A5 |
| Nonce reuse / weak randomness | S | Predictable RNG produces repeated ML-DSA nonces → key recovery | ML-DSA implementation (Dilithium-round3) is deterministic; no application-level nonce generation |
| Fault-injection on signature | T | Glitch during signing produces valid sig leaking private scalar | Out-of-scope for software layer; mitigated at deployment via ECC RAM (documented in `PERFORMANCE_TUNING.md`) |
| Downgrade to classical-only when peer claims no PQ support | E | Negotiation-based attacker forces ECDSA-only session | No negotiation: PQ is mandatory. Peers without ML-KEM support fail handshake |

### 3.2 Residual / open

- **QuantumKeyPair Zeroize wrapper.** The outer wrapper lacks `ZeroizeOnDrop`;
  the inner secret is zeroed but the wrapper's `Clone` on rotation may retain
  copies in rare cases. **Tracked: Phase 1 A5.**
- **Side-channel review of upstream `pqcrypto-*` crates.** We trust the
  vendor's constant-time claims but have not independently validated them on
  x86_64 and aarch64. **Audit deliverable.**

---

## 4. Consensus

**Surface:** `supernova-core/src/consensus/*`,
`node/src/blockchain/{invalidation,checkpoint,fork_resolution}.rs`,
`node/src/validation/*`.

### 4.1 STRIDE

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| 51 % hashrate attack → double spend | S / T | Adversary acquires majority hashrate | Standard PoW assumption §2.2; confirmation-depth recommendations in operator guide; weak-subjectivity checkpoints limit deep rewrites |
| Long-range rewrite past weak-subjectivity | T | CRQC-era adversary re-mines from genesis | Checkpoints are pinned in `supernova-core/src/consensus/checkpoints.rs`; blocks below latest checkpoint are frozen |
| Time-warp attack to lower difficulty | T | Attacker backdates block timestamps across retarget window | Median-time-past (MTP) enforcement in `validation/block.rs`; future timestamps > 2h rejected; retarget floor clamps downward adjustment |
| Selfish mining / block withholding | T | Miner withholds blocks to orphan honest work | Fork-choice v2 (`fork_resolution.rs`): ties broken by hash proximity to difficulty target + earliest-received heuristic; prevents trivial reorg advantage |
| Difficulty manipulation via selective timestamping | T | Colluding miners push timestamps to maximum window | Clamp in `difficulty.rs` caps single-period change; median of last N blocks, not raw |
| Fork-resolution race → state divergence | T / D | Two valid chains arrive simultaneously | Atomic invalidation set with explicit locking (`blockchain/invalidation.rs`); recursive mark-descendants guarded by single writer |
| Division-by-zero in difficulty math | D | Malformed header → panic | All divisions use `checked_div` / `saturating_*`; clippy gate `arithmetic_side_effects` under Phase 2 remediation |
| Coinbase / treasury misallocation | E | Miner keeps treasury share | Consensus-level validation: coinbase must pay treasury address (`miner/src/mining/template.rs`); Phase 1 A1 replaces placeholder with multisig |

### 4.2 Residual / open

- **Treasury multisig genesis.** Treasury address placeholder is still the
  dev constant; **Phase 1 A1** replaces with a governance-controlled m-of-n
  SPHINCS+ multisig before mainnet.
- **Stale-block denial.** An adversary that briefly acquires >50 % can still
  orphan honest blocks; out of scope for protocol, in scope for operator
  monitoring (`docs/operations/runbooks/`).

---

## 5. P2P networking

**Surface:** `node/src/network/*`, `node/src/p2p/*`,
`supernova-core/src/network/quantum_p2p.rs`, `node/src/handshake.rs`.

### 5.1 STRIDE

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| Peer impersonation | S | Attacker spoofs peer identity | libp2p identity keys; handshake authenticated via ML-KEM + signed identity claim |
| Eclipse attack | T / E | Attacker monopolises victim's peer connections | Subnet / ASN / region diversity in `eclipse_prevention.rs`; anchor-peer persistence; minimum outbound diversity thresholds |
| Sybil attack | S / D | Cheap peer creation overwhelms honest peers | Proof-of-work challenge on inbound connection (`peer_identity.rs`); per-IP / per-subnet rate limits; banscore for repeated misbehaviour |
| MITM on handshake | I / T | Downgrade or substitute KEM pubkey | ML-KEM-768 encapsulation binds session key to peer's long-term identity key; handshake transcript signed |
| Message tampering post-handshake | T | Replay or modify P2P messages | libp2p Noise transport provides AEAD with replay-safe nonces |
| Traffic analysis | I | Adversary infers topology / balances from packet sizes | Out of scope v1; padding planned in v1.1 |
| P2P message flood / DoS | D | Spam INV / GETDATA / block headers | Per-message-type rate limits (`network/rate_limiter.rs`); banscore increments on violation; eviction of low-score peers |
| Large-message OOM | D | 32 MB+ message exhausts memory | Hard cap 4 MB per message; per-connection read buffers bounded |
| Invalid-block flood | D | Peer relays sequence of invalid headers | Banscore + connection drop on first invalid header; proof-of-work required before deep validation |

### 5.2 Residual / open

- **Network-wide censorship** under state-level adversary is out of scope;
  Tor/I2P support deferred to v1.1.
- **Padding / timing analysis** defence is v1.1.

### 5.3 Clock skew

Timestamps more than 2 h in the future are rejected at header validation.
MTP is used inside consensus; wall-clock is used only for network-level
rate-limiting. Operators SHOULD run NTP.

---

## 6. Mempool

**Surface:** `node/src/mempool/*`.

### 6.1 STRIDE

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| Mempool DoS via low-fee spam | D | Flood of 1-sat-per-byte txs fills buffer | Minimum fee-rate floor; dynamic adjustment as mempool fills; commit 285244c wired fee-rate cap |
| Fee overflow → negative fee | T | Crafted tx with `outputs > inputs` wraps | `checked_add` / `checked_sub` throughout fee math; tx rejected on overflow |
| RBF pinning | T / D | Cheap descendant tx blocks ancestor replacement | BIP-125-style rules with explicit replacement-relay limits; max 100 descendants |
| Eviction of high-fee tx under memory pressure | T / D | Adversary times eviction to drop victim's tx | Eviction is fee-rate-ordered from lowest; high-fee txs never evicted except on cache overflow of their own ancestor set |
| Admission-time panic on malformed tx | D | Unsafe unwrap in tx validation | Phase 2c panic remediation covers mempool paths |

### 6.2 Residual / open

- **Ancestor/descendant CPU blow-up** under pathological tx graphs: bounded
  at 100 ancestors / 100 descendants; adversary-controlled worst-case stress
  testing scheduled in Phase 5 E5.

---

## 7. Storage / UTXO

**Surface:** `supernova-core/src/storage/utxo_set.rs`, `node/src/storage/*`.

### 7.1 STRIDE

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| Double-spend | T | Same UTXO consumed twice across blocks | Atomic UTXO set (`atomic_utxo_set.rs`); lock-free DashMap/DashSet enforces single consumption per height |
| Storage corruption (disk fault) | T / D | Partial write leaves sled DB inconsistent | sled's WAL + `flush()` on block commit; integrity validation on startup (`validation/checkpoints.rs`) |
| Disk-fill DoS | D | Adversary-sized blockchain exceeds disk | Operators must provision ≥ 512 GB initial; pruning mode planned v1.1 |
| Keystore theft | I / E | Disk image leak | Keystore is Argon2id + ChaCha20-Poly1305 (`wallet/src/quantum_wallet/keystore.rs`); passphrase never persisted |
| Backup tampering | T | Attacker overwrites S3 backup with malformed state | Backups are ML-DSA signed; restore verifies signature before applying (**Phase 1 A3**) |
| Replay of decommissioned state | T | Old sled snapshot re-attached to running node | Snapshot manifest pins block height + hash; restore fails mismatch |

---

## 8. Wallet

**Surface:** `wallet/src/*`, `supernova-core/src/wallet/*`.

### 8.1 STRIDE

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| HD-key predictability | S / I | Weak seed entropy → brute-force of addresses | Seed derived from BIP-39 mnemonic ≥ 128 bits; Argon2id stretch on keystore; PQ KDF = SHA3-512 (§5 of QUANTUM_SECURITY.md) |
| Signing oracle abuse | E | Malicious RPC client asks wallet to sign arbitrary payload | Wallet validates payload type and destination; admin-scoped RPC required; rate limits in `api/rate_limiter.rs` |
| Sidechannel on signing | I | Timing differences reveal key material | ML-DSA signing is constant-time per pqcrypto upstream; we don't branch on key bits |
| Backup exfiltration | I | Wallet backup file on disk | Argon2id + AEAD; backup envelope also signed for tamper-evidence (Phase 1 A3) |
| Replay of stale signed tx | T | Attacker replays old signed tx | Tx nonce + UTXO consumption make replay invalid once original is mined; unmined replays rejected at mempool fee-rate floor |
| Clone wallet key material | E | Wallet file copied to attacker machine | Keystore is passphrase-encrypted; without passphrase, copy is inert |

### 8.2 Residual / open

- **Hardware-wallet support.** Deferred to v1.1. Current wallets are
  software-only; users running on hostile hardware inherit that trust.

---

## 9. Lightning

**Surface:** `supernova-core/src/lightning/*`.

### 9.1 STRIDE

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| HTLC preimage timing compare | I | Variable-time compare on secret preimage leaks byte-by-byte | `subtle::ConstantTimeEq` across all 6 preimage / HMAC sites (Phase 3 C1) |
| Onion HMAC forgery | T / S | Attacker forges HMAC tag byte-by-byte via timing | Constant-time HMAC compare (`lightning/onion.rs:316`) |
| Channel-state replay | T | Old commitment broadcast after revocation | Revocation key + to-self-delay enforced; watchtower monitors and penalises |
| Watchtower collusion | S | Watchtower withholds justice transactions | Multi-watchtower redundancy; client signs independent justice set per tower |
| Backup confidentiality | I | S3 backup reveals channel state | AEAD-encrypted per-channel; signed before upload (Phase 1 A3) |
| Backup integrity | T | Adversary tampers backup | ML-DSA signature verified on restore (Phase 1 A3) |
| Quantum HTLC timeout exploitation (P1 item from RC4) | E | Attacker forces premature channel close | Mitigated per audit RC4 §P1; dual-timeout check |
| Onion packet size amplification | D | Crafted onion causes excessive hop processing | Fixed 1300-byte onion; hop budget capped at 20 |

### 9.2 Residual / open

- **Peer-backup P2P wiring.** Message variants and correlation not yet
  implemented. **Tracked: Phase 1 A2.**
- **S3 backup provider.** Stub needs full `aws-sdk-s3` integration. **Tracked:
  Phase 1 A3.**

---

## 10. Script interpreter

**Surface:** `supernova-core/src/script/*`.

### 10.1 STRIDE

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| Opcode-driven DoS | D | Pathological script burns CPU | Opcode count cap (per-tx budget); stack depth cap; `OP_CODESEPARATOR` usage bounded |
| Stack / altstack overflow | T / D | Deep recursion exhausts memory | Fixed-size stacks with explicit overflow check returning `ScriptError::StackOverflow` |
| Sighash ambiguity | T | Re-interpretable sighash allows fee-bumping attacks | SIGHASH flags strictly parsed; unknown flags rejected |
| Quantum-unsafe opcodes | S | `OP_CHECKSIG` uses classical-only crypto | All signature opcodes route through hybrid verifier (§3); classical-only rejected post-activation |
| Integer overflow in arithmetic opcodes | T | `OP_ADD` overflow changes semantics | `i64` arithmetic with explicit `checked_*`; overflow → script failure |

---

## 11. Mining / PoW

**Surface:** `miner/src/*`, `supernova-core/src/consensus/difficulty.rs`.

### 11.1 STRIDE

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| Block withholding attack | T | Miner hides solved blocks | Outside protocol scope; monitored via reorg-depth alerts |
| Block-template grinding | T | Miner manipulates timestamp / nonce distribution | Coinbase tx must pay canonical treasury; template builder enforces treasury allocation (Phase 1 A1) |
| Header pre-computation | T | Miner pre-computes templates across reorgs | Extranonce rotation bounded; PoW target enforced at validation |
| PoW algorithm substitution | S | Node accepts block with non-SHA3-512 PoW | Hash algorithm is consensus-pinned; mismatched PoW rejected |

---

## 12. RPC / API

**Surface:** `node/src/api/*`, `node/src/api_facade.rs`,
`node/src/api/jsonrpc/*`.

### 12.1 STRIDE

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| Unauthenticated admin call | E | Reachable admin endpoint without JWT | JWT middleware; admin claim required; default bind is localhost |
| Credential stuffing / brute force | S | Repeated auth attempts | Rate limiter (`api/rate_limiter.rs`); exponential backoff |
| Information disclosure via error | I | Stack trace / internal path leaks in error | Errors are mapped to canonical `ApiError` variants with scrubbed messages |
| RPC amplification / DoS | D | Cheap request triggers expensive computation | Per-method cost class; budget enforced per connection |
| Request smuggling / HTTP parser bugs | T / E | Framework CVEs | `axum` + `tower-http`; cargo-audit daily (Phase 3 C5) |
| CORS misconfiguration | E | Browser origins perform state-changing ops | Default CORS is tightly scoped; explicit allow-list |
| `Send + Sync` race in shared facade | T / E | `ApiFacade` accessed across threads unsafely | Compile-time `assert_impl_all!` will be re-enabled (Phase 1 A4); any non-Send field surfaces as hard compile error |

### 12.2 Residual / open

- **`ApiFacade` thread-safety proof.** Assertion is commented out. **Phase 1
  A4** re-enables it and wraps any offending field.

---

## 13. Governance / treasury

**Surface:** `miner/src/mining/template.rs`, planned
`supernova-core/src/governance/treasury.rs`.

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| Single-party treasury seizure | E | One private key signs treasury spends | m-of-n SPHINCS+ multisig (n ≥ 5); on-chain revealed only on spend (Phase 1 A1) |
| Governance vote manipulation | E | Re-used keys / sybil voting | PQ-signed governance proposals; weight capped per identity |
| Coinbase allocation bypass | T / E | Miner constructs coinbase omitting treasury | Consensus-level validation rejects block |

---

## 14. Environmental oracle

**Surface:** `supernova-core/src/environmental/*`,
`node/src/environmental/oracle.rs`.

| Threat | Category | Vector | Mitigation |
|---|---|---|---|
| Single-oracle manipulation | T | Malicious oracle feeds false carbon data | 2f+1 Byzantine aggregation; median + trim |
| Oracle replay | T | Stale data resubmitted | Timestamps signed + monotonic nonces |
| Policy-input panic | D | Zero / NaN from oracle | Validated at aggregation; out-of-range rejected |

---

## 15. Cross-cutting concerns

### 15.1 Supply chain

- cargo-audit runs daily (`.github/workflows/security-audit.yml`).
- cargo-deny gates licenses, advisories, and source registries
  (`deny.toml`).
- SBOM published per release in CycloneDX JSON (Phase 3 C5).
- Trivy scans Dockerfile + built image for CVEs on every relevant PR
  (Phase 3 C5).
- Release artifacts signed with cosign keyless + GPG-signed tags (Phase 4
  D5).

### 15.2 Secrets in memory

- `Zeroize, ZeroizeOnDrop` on all long-lived secret holders.
- Stack-allocated secrets use `zeroize::Zeroizing<_>` wrappers.
- No secret is ever formatted through `Debug` / `Display`; explicit
  `#[derive(Debug)]` is avoided on secret-bearing structs.

### 15.3 Panic safety

- `#![deny(clippy::unwrap_used, expect_used, panic, unimplemented, todo)]`
  at workspace level.
- Violations monotonically decrease across Phase 2 sub-phases (crypto/
  consensus, network/mempool, remainder).

### 15.4 Observability

- Prometheus metrics exposed on `:9090`; alerting rules under `docker/`.
- Structured JSON logging (tracing-subscriber) with PII redaction
  (`node/src/logging/mod.rs`).
- Incident response runbook: `docs/operations/INCIDENT_RESPONSE.md`.

---

## 16. Abuse / misuse cases (non-STRIDE)

| Scenario | Notes |
|---|---|
| Chain split during contentious upgrade | Upgrade activation uses BIP-9-style version bits with super-majority thresholds; social coordination required before activation |
| Key loss recovery | Users MUST back up the 24-word BIP-39 mnemonic; no protocol-level recovery (intentional, Bitcoin-style) |
| Accidental testnet → mainnet address reuse | Network byte differs; validation rejects cross-network addresses |
| Operator runs on malicious hosting | Out of scope; operator guide recommends full-disk encryption + TPM where available |

---

## 17. Findings catalogue and remediation status

See `PRODUCTION_READINESS_ASSESSMENT.md` for the authoritative per-finding
status. Summary:

- P0 (Critical): **3/3 closed**
- P1 (High):     **4/5 closed** (treasury multisig pending — Phase 1 A1)
- P2 (Medium):   **12/12 closed** per RC4 audit
- Panic safety:  Phase 2 in progress (Phase 2a/2b/2c merged)
- External audit: Phase 6 — auditor not yet selected

---

## 18. Audit engagement scope

The Phase 6 external audit is expected to cover, at minimum:

1. Cryptographic correctness of ML-DSA / SPHINCS+ / ML-KEM integrations
   and the hybrid verifier (§3).
2. Consensus correctness: fork resolution v2, difficulty, timestamp, weak
   subjectivity (§4).
3. P2P protocol: handshake, eclipse, Sybil, DoS (§5).
4. Wallet: HD derivation, keystore encryption, signing flows (§8).
5. Lightning: HTLC quantum-safety, channel backup integrity, watchtower
   (§9).
6. Script interpreter: opcode bounds, arithmetic safety (§10).
7. Supply-chain artifacts: signed releases, SBOM, Docker image scan
   (§15.1).

Audit packet contents:
- This document
- `SECURITY_ASSUMPTIONS.md`
- `PRODUCTION_READINESS_ASSESSMENT.md`
- `docs/BASELINE_RC4.md` (build/test/clippy snapshot)
- Current SBOM artifact
- `docs/adr/` (Phase 4 D4, pending)

---

*Maintained by: Supernova Security Working Group.
Review cadence: on every P0/P1 finding and at each minor release.*
