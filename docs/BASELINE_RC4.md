# RC4 Baseline Snapshot

**Captured:** 2026-04-20
**Branch:** `main`
**Commit:** `285244c` (wire mempool fee-rate)
**Toolchain:** `rustc 1.90.0 (1159e78c4 2025-09-14)`, `cargo 1.90.0`
**Purpose:** Phase 0 baseline for the mainnet implementation plan (`~/.claude/plans/ultrathink-and-conduct-an-serialized-fog.md`).
Progress in subsequent phases is measured against the numbers in this document.

---

## 1 Workspace shape

| Crate | Files | LOC (Rust) |
|---|---|---|
| `supernova-core` | 222 | 74,950 |
| `node` | 174 | 54,849 |
| `wallet` | 20 | 4,909 |
| `miner` | 14 | 2,862 |
| `cli` | 12 | 2,223 |
| `quantum_validation` | 1 | 22 |
| `supernova` (binary) | 1 | 3 |
| **Total tracked** | **444** | **139,818** |
| Workspace total (incl. fuzz/tests/examples) | 468 | 142,949 |

Direct dependencies: **902** (from `Cargo.lock` package entries).

Release binaries produced by `cargo build --release --all-features`:
`supernova-node`, `supernova-cli`, `supernova-banner`, `supernova-oracle`,
`miner`, `mine_genesis`, `wallet`, `quantum_validation`.

---

## 2 `cargo build --release --all-features`

Result: **succeeds**.
Warnings: **323** (see `/tmp/supernova_build.log` for full list, not checked in).

Warning categories (representative, by count):
- Unused variables / unused imports — dominant category (~120)
- `hiding a lifetime that's elided elsewhere is confusing` — 4
- Deprecated `libp2p::libp2p_swarm::SwarmBuilder` — ≥2 (now `libp2p::SwarmBuilder`)
- Dead-code assignments (`utxos_checked`, `blocks_checked`) — 2 each
- Multiple associated items never used — 3

All warnings are non-fatal; nothing blocks build.

---

## 3 `cargo test --workspace --release --no-fail-fast`

### Headline numbers

| Metric | Count |
|---|---|
| Suite result blocks | 35 |
| Tests passed | 656 |
| Tests failed | 42 |
| Tests ignored | 13 |
| Failing suites | 6 |
| Individual failing test IDs | **73** |
| Failing target groups (cargo reports) | 7 |

Pass rate (passed / (passed + failed)): **93.98 %**.

### Failing cargo targets

```
-p supernova-node --lib
-p supernova-node --test api_integration_tests
-p supernova-node --test utxo_reorg_tests
-p wallet --lib
-p wallet --bin wallet
-p wallet --test quantum_hd_derivation_tests
-p supernova-node --doc
```

### Deadlock observed

The `node` lib-test binary (`target/release/deps/node-*`) hung in
`blockchain::invalidation::tests::test_descendant_invalidation` and
`network::eclipse_prevention_tests::eclipse_prevention_tests::*`
for >7 minutes at 0 % CPU. Sending `SIGKILL` was required to let the
remainder of the workspace finish. This is a **live deadlock** in the
current RC4 head and must be triaged before RC5; a candidate cause is
lock ordering between the eclipse-prevention state machine and the
connection-flooding detector — flagged for Phase 2b.

### Failing test groups (clusters)

Grouped by subsystem for prioritisation. Full list preserved in
`/tmp/supernova_test.log`.

1. **`mempool` — 11 failures** (dos_protection, manager, mev_protection,
   prioritization, priority_queue). Suggests regression from commit
   `285244c` (fee-rate wiring). Highest blast radius.
2. **`blockchain::checkpoint` + `blockchain::genesis` — 6 failures** plus
   one `blockchain::tests::test_create_genesis_block`. Consensus-critical.
3. **`logging::tests` — 5 failures** for secret-redaction patterns
   (api_key, private_key, seed_phrase, standalone_hex_key, partial_address).
   Direct information-leak risk.
4. **`api::middleware` — 5 failures** (`test_no_auth_bypass`,
   `test_auth_middleware_invalid_key`, `test_rate_limiter_over_limit`,
   `test_retry_after_on_429`, `test_api_config_default`). Auth/rate-limit
   gating does not behave as specified.
5. **`network` — 3 failures** (header-first propagation, compact-block
   reconstruction, eclipse-prevention connection-flooding).
6. **`node/tests/utxo_reorg_tests` — 13 failures** covering 2/5/deep reorgs,
   coinbase handling, wallet balance preservation, double-spend prevention,
   and reorg-exceeds-max-depth. **Consensus-critical.**
7. **`wallet::quantum_wallet` — 6 failures** (address, keystore, storage,
   transaction_builder, plus core wallet tests and password_strength).
8. **`wallet/tests/quantum_hd_derivation_tests` — 11 failures** covering
   child-key derivation, index boundaries, forward secrecy, entropy checks,
   purpose-based derivation.
9. **`node::api_integration_tests` — 6 failures** panicking on the
   `blockchain_info`, `network_info`, `mempool_info`, `block_by_height`,
   `openapi_spec`, and `submit_invalid_transaction` endpoints — the
   integration harness cannot bring up `ApiFacade`, which aligns with the
   commented-out `assert_impl_all!(Send, Sync)` at `node/src/api_facade.rs:40`.
10. **Doctests — 3 failures** in `node/src/storage/checksum.rs` and
    `node/src/mining/merkle.rs` (unresolved `supernova_node`,
    `StreamingChecksum`, `calculate_merkle_root` references — doctest
    imports out of sync with module structure).

### Interpretation

The plan's earlier RC4 pass-rate claim ("96.9 % / 385 passing") is
**inconsistent with current HEAD**. Treat 656 passed / 73 failed as the
authoritative Phase 0 number and phase 1/2 completion must restore
the previously-reported green state as a precondition for testnet.

---

## 4 `cargo clippy --workspace --all-features --release`

Result: **errors out in `supernova-core` before reaching `node`, `wallet`,
`miner`, or `cli`** (workspace-wide clippy gating is blocked by the
panic-class deny lints in `supernova-core`'s own `lib.rs`).

### Counts (supernova-core lib only, partial coverage)

| Type | Count |
|---|---|
| clippy errors (deny-level) | **278** |
| clippy warnings | 272 |

### Deny-level lints triggered (panic-safety)

These are the enforcement targets from `supernova-core/src/lib.rs:5-10`
and `node/src/lib.rs:5-10`:
`#![cfg_attr(not(test), deny(clippy::unwrap_used))]`, `expect_used`, `panic`,
`unimplemented`, `todo`, `unreachable`.

Per-lint site counts in `supernova-core` (alone):

| Lint | Sites |
|---|---|
| `clippy::unwrap_used` | **270** |
| `clippy::expect_used` | **17** |
| `clippy::unreachable` | 1 |
| `clippy::panic` | 1 |
| **Panic-class total** | **289** (supernova-core lib) |

`node` lib clippy is also gated by the same denies but could not be
exercised in this run because `supernova-core` failed first. Raw grep
of `.unwrap() / .expect( / panic!` in non-test production paths:
`supernova-core` 843, `node` 392, `wallet` 108, `miner` 19, `cli` 15.
These include sites that clippy does not flag (e.g. in tests or under
`cfg(test)`), so the clippy-enforced figure is strictly smaller.

The plan's "243 violations" estimate was low. **Phase 2 budget: ≥289
confirmed panic-class violations in `supernova-core`, plus whatever
`node`/`wallet`/`miner`/`cli` contribute once upstream errors are cleared.**

### Top non-panic warnings (by frequency)

| Lint | Count |
|---|---|
| `needless_borrows_for_generic_args` | 28 |
| `field_reassign_with_default` | 11 |
| `empty_line_after_doc_comments` | 8 |
| `should_implement_trait` | 5 |
| `needless_range_loop` | 5 |
| `collapsible_if` | 3 |
| `unnecessary_map_or` | 3 |
| `op_ref` | 3 |
| `arc_with_non_send_sync` | 2 |
| `new_without_default` | 2 |

`arc_with_non_send_sync` (2 sites) is the most relevant to the RPC
`Send + Sync` work in Phase 1 A4 — worth grepping first when that
track starts.

---

## 5 `cargo audit` — **BLOCKED**

Version: `cargo-audit-audit 0.21.2`.

```
error: error loading advisory database: parse error: error parsing
/Users/marcjohnson/.cargo/advisory-db/crates/libcrux-poly1305/RUSTSEC-2026-0073.md:
parse error: TOML parse error at line 5, column 8
  cvss = "CVSS:4.0/AV:N/AC:L/AT:N/PR:N/UI:N/VC:N/VI:N/VA:H/SC:N/SI:N/SA:N"
  unsupported CVSS version: 4.0
```

This is a known cargo-audit bug: the advisory database now contains
CVSS 4.0 scores which `cargo-audit` ≤0.21.2 cannot parse. **Upstream fix
in ≥0.21.3.** Baseline CVE count therefore unobtained.

**Action (Phase 3 C5 prerequisite):** `cargo install cargo-audit --locked`
to pick up the fix, then re-run and record findings.

---

## 6 TODO / FIXME / `unimplemented!` inventory

```
TODO + FIXME in *.rs (workspace, incl. tests) : 35
unimplemented!() / todo!() macros              :  0 (confirmed — only comment hits)
```

Files with outstanding markers (19):

```
miner/src/mining/template.rs                            # treasury placeholder
miner/tests/treasury_validation_tests.rs
node/src/api/lightning_api.rs
node/src/api/server.rs
node/src/api_facade.rs                                  # Send+Sync disabled
node/src/lib.rs
node/src/mempool/manager.rs
node/src/miner/block_producer.rs
node/src/network/block_propagation.rs
node/src/storage/checkpoint.rs
node/src/storage/database_shutdown.rs
node/src/validation/parallel_validator.rs
node/src/validation/sig_cache.rs
node/tests/utxo_reorg_tests.rs
supernova-core/src/atomic_swap/api.rs                   # refund stubs
supernova-core/src/config.rs
supernova-core/src/lightning/backup.rs                  # peer-send + S3 stubs
supernova-core/src/mempool/transaction_pool.rs
supernova-core/tests/atomic_swap_rollback_tests.rs
```

This is the ground-truth list for the plan. The earlier audit's claim
of thousands of outstanding items is not supported by the codebase.

---

## 7 Orphan duplicate-file cleanup (Phase 0 A2)

All duplicate `<name> 2.rs` files in the source tree were deleted in
the prior session — none remain under version-controlled paths:

```
$ git grep -l " 2\.rs"
(no matches)
$ find . -name "* 2.rs" -not -path "./target/*"
./quantum_validation/target/release/build/typenum-*/out/tests 2.rs   # ignored: cargo build output
```

The surviving hit is a generated `typenum` build artifact inside
`quantum_validation/target/` and is not a source file.

Currently staged:

```
D  .clippy.toml.strict        # strict lint config removed
?? .clippy.toml.toml           # accidental rename artifact — left in place for user review
```

---

## 8 Clippy-config state (noted, not yet acted upon)

- `.clippy.toml` (tracked, 445 bytes): current pragmatic config
  (complexity 35, 150 lines/fn, 10 args, MSRV 1.70).
- `.clippy.toml.strict` (tracked, deleted in worktree): former strict
  profile that explicitly disallowed `unwrap/expect/Index`.
- `.clippy.toml.toml` (untracked, 1528 bytes): same content as the
  deleted strict profile — appears to be a `mv` accident when the
  strict file was renamed. Keep for reference until Phase 2 picks a
  canonical strict-mode toggle (e.g., a CI-only alt-config path).

Panic-safety enforcement today is in per-crate `lib.rs` attributes
(`#![cfg_attr(not(test), deny(clippy::unwrap_used))]` etc.), **not**
in any `.clippy.toml`. Those attributes are what generate the 278
errors recorded above.

---

## 9 Phase 0 exit criteria

| Criterion | Status |
|---|---|
| Build baseline captured | ✅ 323 warnings, 0 errors |
| Test baseline captured | ✅ 656 passed / 73 failed / 13 ignored, deadlock noted |
| Clippy baseline captured | ✅ 278 errors (panic-class 289) in supernova-core |
| Audit baseline captured | ⚠️ blocked by cargo-audit 0.21.2 CVSS 4.0 bug |
| Machete (unused deps) | ⏭️ skipped — tool not installed, deferred to Phase 3 C5 |
| Duplicate source files removed | ✅ none remain under tracked paths |
| Metrics file written | ✅ this document |

Phase 0 is complete in the sense of *measurement* — the numbers above
are the starting line. Phase 1 may begin.

---

## 10 Delta the plan must absorb from this baseline

1. Panic-class violation budget revised **243 → ≥289** (supernova-core
   alone). Phase 2a scope grows proportionally.
2. Test pass rate revised **96.9 % → 93.98 %**. Thirteen UTXO-reorg
   tests and six genesis/checkpoint tests are **failing on HEAD**;
   these are consensus-critical and must be restored before testnet,
   not merely maintained.
3. A **hard test deadlock** exists in the node lib suite
   (`blockchain::invalidation` + `eclipse_prevention_tests`). Add as a
   Phase 2b track before (or coincident with) the eclipse-prevention
   panic remediation.
4. `cargo-audit` must be upgraded before Phase 3 C5 — not a new
   sub-task, but a blocker the plan did not call out.
5. The commented-out `assert_impl_all!(ApiFacade: Send, Sync)` at
   `node/src/api_facade.rs:40` correlates with the six
   `api_integration_tests` panics — Phase 1 A4 will likely resolve
   both together.
