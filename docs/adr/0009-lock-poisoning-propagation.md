# ADR-0009: Lock-poisoning propagation and poison-recovery conventions

**Status:** Accepted
**Date ratified:** 2026-04 (Phase 2 panic-safety remediation,
  workspace-wide; commits in the `propagate <module> poison` series)
**Scope:** Workspace-wide coding convention / panic-safety policy

---

## Context

`supernova-core/src/lib.rs` and `node/src/lib.rs` enforce
`#![cfg_attr(not(test), deny(clippy::{unwrap_used, expect_used, panic,
unreachable, todo, unimplemented}))]`. That deny lint elevates any
panic-class call in production code to a hard build error.

Most of the workspace's panic-class violations were `Mutex::lock()
.unwrap()` and `RwLock::{read, write}().unwrap()` — Rust's poisoning
sentinel. A `RwLock` becomes "poisoned" when a thread panics while
holding the write guard; the next acquire returns `Err(PoisonError<...>)`
rather than the guard itself. The default recovery is `.unwrap()`, which
re-panics — exactly the production-panic the lint is here to prevent.

We had thousands of such sites across:

- `supernova-core::{lightning, atomic_swap, environmental, security,
  monitoring, testnet, types, util, storage, mining, wallet, ...}`
- `node::{api, network, mempool, ...}`
- A handful in `wallet`, `miner`, `cli`.

The ad-hoc fix pattern (`if let Ok(g) = lock.read() { ... } else
{ panic!("poisoned") }`) was tempting but doesn't scale: every site
needs a consistent policy, every reviewer needs to know the policy,
and every future contributor introducing a new lock needs to follow
the same shape — otherwise the lint will stop them at the door but
they won't know why.

We needed a **single workspace-wide convention** with two clearly-
distinguished cases.

## Decision

**Two patterns**, chosen by the function's return type. Both treat
`PoisonError` deterministically; neither panics on production builds.

### Pattern A — `Result`-returning paths: propagate as typed error

If the function already returns `Result<_, ModuleError>` for any
reason, lock acquisition propagates a new `LockPoisoned` variant of
the same error enum:

```rust
#[derive(thiserror::Error)]
pub enum LightningNetworkError {
    // ... existing variants ...

    #[error("Internal lock poisoned — another thread panicked while holding the lock")]
    LockPoisoned,
}

pub fn open_channel(&self, ...) -> Result<ChannelId, LightningNetworkError> {
    let mut channels = self
        .channels
        .write()
        .map_err(|_| LightningNetworkError::LockPoisoned)?;
    // ... normal logic ...
}
```

Every error enum in the workspace gets a `LockPoisoned` variant when
it first acquires a poisoned-able lock. The error message is
identical across crates ("Internal lock poisoned — another thread
panicked while holding the lock") so operators see consistent log
output regardless of which subsystem failed.

### Pattern B — non-`Result` read-only accessors: poison-recover

If the function returns `Vec<...>`, `Option<...>`, `bool`, etc., and
its purpose is read-only inspection of state, recovery via
`PoisonError::into_inner()` is preferred over a signature change:

```rust
pub fn list_channels(&self) -> Vec<ChannelId> {
    let channels = self
        .channels
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    channels.keys().cloned().collect()
}
```

The recovered guard still dereferences to the protected data;
poisoning is an advisory flag, not corruption. For pure reads,
proceeding with the data we have is strictly better than refusing
to answer at all (which would force every caller to handle a
result they cannot meaningfully recover from).

### When to use which

| Function shape | Pattern |
|---|---|
| `fn ... -> Result<_, ModuleError>` | A — propagate `LockPoisoned` |
| `fn ... -> Option<_>` (read-only accessor) | B — poison-recover |
| `fn ... -> Vec<_>` / `bool` / `&T` (read-only) | B — poison-recover |
| `fn ... -> ()` that mutates shared state | A — change to return `Result` |
| `fn ... -> ()` for fire-and-forget metrics | B — poison-recover with `tracing::warn!` |

### `SystemTime` and `bincode::serialize` — adjacent cases

Two patterns we encountered repeatedly during the same remediation:

- **`SystemTime::now().duration_since(UNIX_EPOCH).unwrap()`** —
  fails only on a pre-1970 wall-clock. Convert to
  `.unwrap_or(Duration::ZERO)`. Downstream consensus rules reject
  bogus timestamps, so the failure surfaces at the validator instead
  of crashing the constructor.
- **`bincode::serialize(self).unwrap()`** on hot-path consensus
  hashers (`Transaction::hash`, `Block::serialize`) — provably
  infallible for standard `Serialize` derives. Convert to
  `.unwrap_or_else(|e| { tracing::error!(...); Vec::new() })` with
  an explicit doc note on why the failure arm is unreachable. The
  empty-vec fallback yields the SHA-256-of-empty constant
  `e3b0c4429…`, a recognisable on-inspection sentinel if it ever
  appears in production logs.

Both follow the same principle: don't panic in production, surface
the failure to a layer that can react.

## Consequences

### Positive

- **The `deny(clippy::*)` lint is genuinely enforced.** A future
  contributor introducing a new lock cannot land code that compiles
  with `unwrap()` on `lock()`; the build breaks at PR time. Pattern
  is uniform enough that fixing the build error is mechanical.
- **Operator-visible diagnostics are consistent.** Every
  `LockPoisoned` log line reads the same; ops dashboards can pivot
  on a single string.
- **Tests aren't affected.** The lint is `deny` only outside `cfg(test)`
  (`#![cfg_attr(test, allow(clippy::*))]` is set in the same lib.rs),
  so test fixtures keep using `lock().unwrap()` for brevity. Production
  paths are policed; test paths are pragmatic.
- **No `Mutex::lock()` panic ever reaches end users.** A poisoned
  lock surfaces as either a typed RPC error (Pattern A) or
  best-effort degraded read (Pattern B), never as a crashing process.
- **Refactor was tractable.** Workspace went from ~200 panic-class
  clippy violations in `supernova-core` (and 1 cascading violation
  in `wallet`) to **0 errors workspace-wide** by mechanical
  application of the two patterns above. Every change was reviewable
  in isolation; commits were named `propagate <module> poison`.

### Negative

- **Error-enum proliferation.** Every module's error enum now has a
  `LockPoisoned` variant. There are ~15 of these across the
  workspace. Could be consolidated into a single
  `crate::SharedError::LockPoisoned`, but that would couple every
  module's error type to a workspace-shared crate, which is heavier
  than the duplication is. Accepted.
- **Pattern B masks real bugs.** A poisoned lock means *some other
  thread panicked while holding it*. Pattern B silently keeps
  serving reads from that state. If the panic corrupted invariants
  in the protected data, downstream readers may produce wrong
  answers. Mitigation: panic-class lints are denied workspace-wide,
  so the panicking-while-holding-lock case is itself far less likely
  than in a typical Rust codebase. But not impossible.
- **Pattern A occasionally cascades.** A `()`-returning function that
  internally needed lock access had to be widened to return
  `Result<(), ModuleError>`. Several such cascades happened during
  the remediation. Each was small but they touched a lot of files.
- **Test/production divergence.** A test that exercises the
  `lock().unwrap()` path can pass against a build that, under
  `cfg(not(test))`, takes the `?` path instead. Mitigation: this is
  the same testing-risk class flagged in [ADR-0008](0008-bulletproof-range-proof-fail-closed.md);
  `cfg(test)` allow + `cfg(not(test))` deny is the project-wide
  convention.

### Alternatives considered

- **`unwrap()` everywhere with `unwrap_or_else(|_| panic!(...))`.**
  Rejected — same lint failure, just more verbose. The panic is the
  problem, not the call site shape.

- **Replace `std::sync::{Mutex, RwLock}` with a non-poisoning crate
  (e.g. `parking_lot`).** Considered. Reasons we didn't: (1) we'd
  still need to handle the case where a thread panics while
  holding the lock — `parking_lot` doesn't poison but it doesn't
  *prevent* corruption either; (2) workspace already uses `tokio::
  sync::{Mutex, RwLock}` in many places (which also don't poison),
  alongside `std::sync` in others, and unifying on parking_lot
  was scope creep; (3) the deny lint catches the actual hazard
  (production panics) regardless of which lock library is in play.
  Open to revisit; not load-bearing for the panic-free promise.

- **Always silently recover from poison.** Rejected for write paths.
  Recovering and proceeding with a write under a possibly-corrupted
  lock state is strictly worse than failing the operation: the
  caller has no way to know whether their write hit corrupt
  invariants. Pattern A's fail-loud is the right default for
  mutating paths.

- **Always propagate poison as a `Result` error.** Rejected for
  read-only accessors. Forcing every caller of `list_channels` /
  `get_height` / `is_synced` to handle a `Result` they cannot
  meaningfully recover from is API noise without safety value.
  Pattern B keeps the read path uncluttered.

- **Single workspace-wide error type with a `LockPoisoned` variant.**
  Considered. Rejected because: (1) most module error enums also
  carry domain-specific variants (`ChannelError(ChannelError)`,
  `RoutingError(RoutingError)`, etc.) that don't belong in a
  shared crate; (2) `From<PoisonError<...>>` impls would need to be
  generic over the lock's protected type, creating awkward `where`
  clauses on every error type. The minor duplication of 15
  identical enum variants is cheaper than the type plumbing.

## References

- Workspace `lib.rs` files (`supernova-core/src/lib.rs:5-10`,
  `node/src/lib.rs`) — the deny-lint policy that motivates this ADR.
- Sample sites following Pattern A:
  - `supernova-core/src/lightning/mod.rs` — `LightningNetworkError::LockPoisoned`
  - `supernova-core/src/environmental/oracle.rs` — `OracleError::LockPoisoned`
  - `supernova-core/src/security_mitigation.rs` — `SecurityError::LockPoisoned`
  - `supernova-core/src/environmental/verification.rs` —
    `VerificationError::LockPoisoned`
  - `supernova-core/src/security/quantum_canary.rs` — `CanaryError::LockPoisoned`
- Sample sites following Pattern B:
  - `supernova-core/src/lightning/mod.rs::list_channels` —
    `unwrap_or_else(|p| p.into_inner())` on `RwLock::read()`.
  - `supernova-core/src/environmental/oracle.rs::cache_verification`
    — best-effort poison-recover on a metrics write.
- `CHANGELOG.md` `[Unreleased]` → "Changed" — workspace-wide
  panic-class clippy remediation entry; lists the closed-out paths.
- [ADR-0008](0008-bulletproof-range-proof-fail-closed.md) —
  companion `cfg(not(test))` divergence pattern (different decision
  but same testing-risk class noted here).
