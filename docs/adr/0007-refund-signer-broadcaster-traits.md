# ADR-0007: Refund-flow trait abstraction for atomic-swap RPC

**Status:** Accepted
**Date ratified:** 2026-04 (Phase 3 C4, commits `build refund tx` →
  `add refund broadcaster` → `demo refund flow`)
**Scope:** Atomic-swap RPC / cross-crate dependency design

---

## Context

`AtomicSwapRpcImpl::refund_swap` (in
`supernova-core/src/atomic_swap/api.rs`) needed to evolve from a stub
returning a sentinel string (`STUB_refund_<id>`) to a flow that:

1. Builds a consensus-correct unsigned refund transaction.
2. Signs it with the initiator's quantum-resistant secret key.
3. Broadcasts the signed tx to the Supernova p2p network.

Step 1 is pure logic over the HTLC. Steps 2 and 3 require access to
state that lives outside the `supernova-core` crate:

| Resource | Crate | Why outside core |
|---|---|---|
| Quantum keys for `htlc.initiator.address` | `wallet` (via `node`'s `WalletManager`) | core has no key custody; that's a wallet concern |
| Mempool / peer fanout | `node` (via `NetworkProxy`) | core has no p2p stack; that's a node concern |

`supernova-core` cannot directly depend on `node` or `wallet`: those
crates already depend on `supernova-core`, so a back-edge would create
a dependency cycle. That rules out the obvious "just import
`WalletManager`" approach.

We also had a constraint from the existing call sites: `AtomicSwapRpcImpl::
new` is invoked from **8+ test fixtures** plus a handful of tooling
sites. Adding new required arguments would cascade through all of them.

## Decision

**Two `async_trait` abstractions defined in `supernova-core`, with
implementations expected to live outside core (in `node`).** The RPC
takes optional `Arc<dyn Trait>` handles, wired in via a fluent builder
pattern. Errors are typed enums so the JSON-RPC layer can map them to
distinct error codes without parsing strings.

```rust
// supernova-core/src/atomic_swap/api.rs

#[async_trait::async_trait]
pub trait RefundSigner: Send + Sync {
    async fn sign_refund(
        &self,
        htlc: &SupernovaHTLC,
        message: &[u8],
    ) -> Result<MLDSASignature, RefundSignerError>;
}

#[async_trait::async_trait]
pub trait RefundBroadcaster: Send + Sync {
    async fn broadcast_refund(
        &self,
        tx: &Transaction,
    ) -> Result<(), RefundBroadcastError>;
}

pub enum RefundSignerError {
    AddressNotFound(String),  // wallet doesn't hold this key
    SigningFailed(String),    // HSM down / derivation failure / etc.
}

pub enum RefundBroadcastError {
    Rejected(String),  // mempool policy refused (fee, double-spend)
    Network(String),   // peer unreachable / p2p stack uninitialised
}
```

`AtomicSwapRpcImpl` carries `Option<Arc<dyn RefundSigner>>` and
`Option<Arc<dyn RefundBroadcaster>>` fields, defaulting to `None`. A
fluent builder wires them in:

```rust
let rpc = AtomicSwapRpcImpl::new(config, monitor, bitcoin_client)
    .with_refund_signer(Arc::new(NodeWalletSigner::new(wallet_mgr)))
    .with_refund_broadcaster(Arc::new(NodeNetworkBroadcaster::new(net_proxy)));
```

`refund_swap` consults the optional handles:

- No signer wired → tx built unsigned, txid returned, audit log notes
  `signed=false`. (Legacy stub-mode behaviour, preserved.)
- Signer wired, signing succeeds → tx rebuilt with signature in the
  input's `signature_script`; the *signed* tx's `Transaction::hash()`
  is the returned txid.
- Signer wired but `sign_refund` fails → log a warning, fall through to
  unsigned-mode return (same shape as no-signer-wired case).
- Broadcaster wired AND signed=true → call `broadcast_refund`;
  log success or failure but don't fail the RPC over a broadcast
  hiccup (the txid is still useful to the caller for retry).

`SupernovaHTLC::create_refund_message` was raised from `pub(crate)` to
`pub` so external `RefundSigner` implementations can produce a
signature over the same canonical bytes (`REFUND:<htlc_id>:<absolute_
timeout>:<amount>`) that `verify_refund` will later check.

## Consequences

### Positive

- **No dependency cycle.** `supernova-core` defines the contract;
  `node` (or any future implementer) supplies the impl. The dependency
  graph stays a DAG.
- **Backwards-compatible at the call site.** Existing `new(config,
  monitor, bitcoin_client)` callers — including 8+ test fixtures — get
  a working `AtomicSwapRpcImpl` that just doesn't sign or broadcast.
  No cascading edits required.
- **Test-friendly.** Tests can inject mock signer/broadcaster impls
  trivially. The `examples/refund_flow_demo.rs` example does exactly
  this; it's runnable on any developer laptop without infrastructure.
- **Typed errors flow through the RPC layer.** JSON-RPC error codes
  for "wallet doesn't manage this key" vs "HSM unreachable" vs
  "mempool rejected" are distinct, so client retry logic can be
  precise without parsing log strings.
- **Sign-then-broadcast is a single critical-path code path.** No
  duplication between the "stubbed" and "wired" paths; the same
  `refund_swap` produces the right behaviour at every wiring level.
- **Future broadcaster impls are pluggable.** A test broadcaster that
  records txs to disk for replay, a delayed-broadcast for fee-bumping,
  or a multi-peer parallel broadcaster all fit the same trait without
  changes to `refund_swap`.

### Negative

- **Indirection cost.** Every refund call now goes through a virtual
  dispatch (`Arc<dyn Trait>` is a fat pointer). Negligible against the
  cost of MLDSA signing or a network round-trip; called out for
  completeness.
- **The signed-tx txid differs from the unsigned-tx txid.** When a
  signer is wired, the txid we return changes. Callers who held onto
  the unsigned txid from a previous code path would see mismatched
  identifiers. Mitigated by the fact that this code path was returning
  a `STUB_refund_<id>` sentinel before, so no external system has a
  legitimate reason to retain the pre-signing txid.
- **Two opt-in handles, not one.** A consumer who wires only a signer
  but no broadcaster gets a signed tx that is never sent. The audit
  log makes this state visible (`signed=true, broadcast=false`), but
  it's possible to footgun. Future work may add a single
  `RefundDriver` trait that combines both, once we have real impls
  in-tree.
- **`pub(crate)` → `pub`** on `create_refund_message` widens the
  public API surface slightly. The function is small and stable; the
  cost is low.

### Alternatives considered

- **Move `WalletManager` and `NetworkProxy` into `supernova-core`.**
  Rejected — those types pull in storage, p2p, and side-effecting
  state that core must remain free of. Doing this would inflate every
  consumer of `supernova-core` (including the wallet-only crates).

- **Hard-code an `if cfg!(feature = "node-impls")` branch in
  `refund_swap` that imports node types directly.** Rejected — would
  introduce a circular feature-gated dependency that breaks
  `cargo check` outside that feature. Linters and IDE tooling would
  see broken paths in the default build.

- **Pass a `&dyn Wallet`-shaped concrete handle.** Rejected — same
  layering problem as the previous bullet, just hidden behind a
  trait object whose definition still pulls in node types.

- **Synchronous traits.** Rejected — both signing (HSM round-trip) and
  broadcasting (network call) are inherently async. Forcing
  `block_on` on an async impl would deadlock when the caller is
  itself in an async context (which the JSON-RPC server is).

- **A single `RefundDriver` trait that combines both methods.**
  Rejected for now — a real-world wallet impl is unlikely to share
  state with a real-world broadcaster, and the optionality of each is
  useful for partial-deployment scenarios (test environments often
  want a signer but no broadcaster, or vice versa). Open to revisit
  after both impls are in-tree.

- **Add the handles as required `new()` arguments instead of via a
  builder.** Rejected — would force the 8+ existing test fixtures to
  thread mock handles even when they don't exercise the refund path.
  The builder pattern keeps the existing call sites untouched while
  letting production wiring be one extra `.with_refund_*(...)` line.

## References

- `supernova-core/src/atomic_swap/api.rs` — trait definitions,
  `AtomicSwapRpcImpl`, `refund_swap` integration.
- `supernova-core/src/atomic_swap/htlc.rs` —
  `build_refund_transaction`, `create_refund_message`.
- `supernova-core/src/atomic_swap/mod.rs` — `SwapSession.
  funding_outpoint`, `FundingOutpoint`.
- `supernova-core/examples/refund_flow_demo.rs` — end-to-end working
  example with mock impls; runnable via
  `cargo run --example refund_flow_demo --features atomic-swap`.
- [ADR-0001](0001-post-quantum-algorithm-selection.md) — primitive
  choice (ML-DSA / SPHINCS+ rationale).
- [ADR-0006](0006-treasury-governance.md) — companion governance ADR
  (set during the same Phase 1/3 effort).
- `CHANGELOG.md` `[Unreleased]` — Added/Changed entries for the trait
  trio and the refund-flow demo.
