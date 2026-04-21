# Wallet integration guide

This guide documents how to integrate with Supernova from a wallet, custodial
service, or exchange. It covers:

1. Address format and how addresses are derived from public keys
2. The in-process keystore model and its on-disk semantics
3. HD derivation — what it is and, importantly, what it is *not*
4. The signing flow: hashes, sighash, and signature envelope
5. Multi-signature support (reserved, not yet shipped)
6. A Rust SDK example using the `wallet` crate directly
7. A language-neutral example that talks to a running node over HTTP

All references are to code paths at the current release; line numbers may drift
across releases, so prefer the named functions over the numbers.

> This document is deliberately honest about which primitives are production
> ready and which are scaffolding for future work. Reading the "Known
> limitations" section before implementing anything is strongly recommended.

---

## 1. Address format

Supernova addresses are Bech32m-encoded strings with the human-readable part
`nova`:

```
nova1qv7ype9ew29l9x3wp3lh6k7y9crz72mgg3a8x5p5g5d…
```

### Derivation

```
pubkey_hash = SHA3-512(public_key)[..32]
address     = bech32m("nova", to_base32(pubkey_hash))
```

The source is `wallet/src/quantum_wallet/address.rs::Address::from_public_key`.
Addresses always encode exactly 32 bytes of payload, regardless of which
signature scheme produced the underlying public key.

### Properties

- **Case-insensitive but lowercase canonical.** Bech32m enforces this.
- **Checksum-protected.** Invalid characters or substitutions are rejected on
  parse.
- **Algorithm-agnostic.** The scheme that generated the public key is not
  visible in the address itself. The scheme is carried on every transaction
  signature (`TransactionSignatureData::scheme`) and bound there, so a wallet
  that signs with ML-DSA and a wallet that signs with SPHINCS+ both produce
  `nova1…` addresses.
- **No "script" addresses yet.** The `AddressType` enum has `Multisig` and
  `ScriptHash` variants, but only `Standard` is produced by the current
  wallet. See §5.

### Parsing

Any integration that parses addresses should use the Bech32m decoder and
reject:

- HRPs other than `nova`
- Variants other than Bech32m (Bech32 v1 is explicitly rejected)
- Decoded payloads of any length other than 32 bytes

The reference implementation is `Address::from_str` in the same file.

---

## 2. The keystore

`wallet::quantum_wallet::Keystore` is the in-process container for keypairs.
Each entry is a `KeyPair` holding an ML-DSA (Dilithium5) public key, secret
key, derived address, optional label, and creation timestamp.

### Passphrase and locking

- Keystores are locked by default on construction.
- `initialize(passphrase)` generates a 64-byte master seed from `OsRng`,
  stores the Argon2id PHC string of the passphrase, and unlocks the keystore
  in-process.
- Argon2id parameters follow OWASP minimums: 64 MiB memory, 3 iterations, 4
  threads.
- `unlock(passphrase)` verifies against the stored PHC string and flips the
  lock.
- `lock()` flips the lock back without clearing keypairs from memory. For
  stronger guarantees, drop the `Keystore` — `KeyPair::drop` zeroises secret
  material.

### On-disk encoding

This repository does not yet ship a serialised keystore file format. Wallets
that persist the keystore to disk are responsible for encrypting the
`master_seed` and `keypairs` under a key derived from the passphrase, and for
zeroising plaintext after write. Do not persist the PHC string and the
plaintext keys to the same artifact.

### Signing primitives

```rust
let sig = keypair.sign(message)?;
let ok  = KeyPair::verify(&keypair.public_key, message, &sig);
```

`sign` produces a detached Dilithium5 signature (4595 bytes). `verify` is a
static method — it works against any `(public_key, message, signature)`
triple, regardless of whether a keystore is present.

---

## 3. HD derivation

> **This section is the most important one for exchanges and custodians.
> Read it before building any recovery workflow.**

Supernova does **not** implement BIP-32, and cannot. BIP-32's child-key
derivation rides on the algebraic structure of secp256k1; post-quantum
signatures have no such structure, so the scheme here is a custom,
Supernova-specific construction.

### Two derivation paths

| API | What it is | Stability |
|---|---|---|
| `QuantumHDDerivation::derive_child_key(index)` | Derive a 64-byte seed from master seed + index, mixing in fresh system entropy each call. | **Non-deterministic.** See below. |
| `QuantumHDDerivation::derive_for_purpose(purpose, index)` | Derive a purpose-namespaced seed. Calls `derive_child_key` internally. | Inherits non-determinism. |

The implementation lives in `wallet/src/quantum_wallet/hd_derivation.rs`.

### The non-determinism caveat

The docstring on `derive_child_key` claims "same seed + index = same key (for
backup/recovery)". The implementation violates that claim: step 2 of the
derivation mixes 32 bytes of fresh `OsRng` output into the hash state on
**every call**, so repeated derivations for the same `(seed, index)` produce
different output.

This is intentional in the current code — the comment there describes it as a
forward-secrecy property — but it is incompatible with typical HD-wallet
recovery workflows that expect derivation to be reproducible from a mnemonic.

**What this means for integrators:**

- Do not design a "recover my wallet from the seed phrase" flow on top of the
  current `derive_child_key`. It will not reproduce the same addresses.
- Persist every derived key to the keystore at creation time. Treat the
  master seed as protection against root-secret exposure, **not** as the
  single source of truth for recreating the address set.
- A deterministic variant is tracked for a future release. Until it lands,
  the keystore snapshot is the authoritative source for key material.

### Input validation the derivation performs

- Minimum master-seed length: 32 bytes (256 bits).
- Entropy quality gate: rejects all-zero, all-`0xFF`, all-same-byte, or
  >12.5% zero/`0xFF` bytes in the master seed.
- Maximum derivation index: `0x7FFFFFFF`.

These are enforced in `QuantumHDDerivation::from_seed` and `derive_child_key`.

---

## 4. Signing flow

### The sighash

A transaction sighash is computed as:

```
sighash = SHA-256(bincode::serialize(&transaction))
```

Note: **SHA-256, not SHA3.** The rest of the protocol uses SHA3-512 for
addresses and HD derivation, but the transaction digest is plain SHA-256. The
serialisation is the Rust `bincode` encoding of the `Transaction` struct as
defined in `supernova-core/src/types/transaction.rs`.

### The signature envelope

After signing, the resulting detached signature is wrapped in a
`TransactionSignatureData`:

```rust
TransactionSignatureData {
    scheme:         SignatureSchemeType::Dilithium,
    security_level: 5,
    data:           signature,        // 4595 bytes for Dilithium5
    public_key:     keypair.public_key.clone(),
}
```

Other supported `scheme` values are `Ed25519`, `Falcon`, `SphincsPlus`, and
`Hybrid`. The wallet crate currently only produces `Dilithium` envelopes; the
others are accepted by the verifier where implemented.

### Known limitations

1. **Per-input signing is not wired up.** The current builder at
   `wallet/src/quantum_wallet/transaction_builder.rs` signs the sighash with
   the keypair of the **first** selected input and attaches a single
   signature envelope to the whole transaction. Multi-input transactions
   where the inputs belong to different keypairs will fail validation on a
   real node. This is a known scope item, tracked in the transaction-builder
   source comments.

2. **Hybrid signature verification is a stub.** A `Hybrid` envelope passes
   parsing but the verifier currently returns `false` unconditionally (see
   `supernova-core/src/types/transaction.rs`, hybrid verification branch).
   Do not ship Hybrid-mode signing against a Supernova node until that is
   wired up.

3. **Witness/script formats are not yet exposed through the wallet API.**
   Internally the codebase distinguishes P2PKH/P2SH/P2WPKH/P2WSH-style
   scripts, but the wallet's standard address flow commits to a pubkey hash
   and lets the verifier derive the scheme from the signature envelope.

---

## 5. Multi-signature

The `AddressType` enum reserves `Multisig` and `ScriptHash` variants, but
these are not constructed anywhere in the current keystore or transaction
builder. An integration that needs m-of-n signing today must compose it
out-of-band (for example, pre-aggregated signatures under a single address)
and cannot rely on a script-level OP_CHECKMULTISIG equivalent being honoured.

A native multi-signature flow is on the roadmap alongside the deterministic
HD derivation rework. Until then, treat addresses as single-signature only.

---

## 6. Rust SDK example

This uses the `wallet` crate directly. It is the shortest path to a signed
transaction — and the only path that does not go through the HTTP API.

```rust
use std::sync::Arc;

use wallet::quantum_wallet::address::Address;
use wallet::quantum_wallet::keystore::Keystore;
use wallet::quantum_wallet::transaction_builder::{
    BuilderConfig, CoinSelectionStrategy, TransactionBuilder,
};
use wallet::quantum_wallet::utxo_index::Utxo;

fn send_payment(
    passphrase: &str,
    available: Vec<Utxo>,
    recipient: &str,
    amount_attonovas: u64,
) -> anyhow::Result<Vec<u8>> {
    // 1. Open and unlock the keystore.
    let mut keystore = Keystore::new();
    keystore.initialize(passphrase)?;

    // 2. Produce two addresses: one for the change, one to receive.
    let change_addr  = keystore.generate_address(Some("change".into()))?;
    let _funding     = keystore.generate_address(Some("funding".into()))?;

    // 3. Parse the destination address. A well-formed `nova1…` string is
    //    required; anything else is rejected by the Bech32m decoder.
    let to = Address::from_str(recipient)?;

    // 4. Build and sign a transaction.
    let keystore = Arc::new(keystore);
    let mut builder = TransactionBuilder::new(keystore, BuilderConfig {
        fee_rate:       1_000,  // attonovas / byte
        min_fee:        10_000,
        max_tx_size:    100_000,
        coin_selection: CoinSelectionStrategy::BranchAndBound,
        dust_threshold: 546,
    });

    builder.set_change_address(change_addr);
    builder.add_output(to, amount_attonovas)?;
    builder.select_coins(&available)?;
    let tx = builder.build_and_sign()?;

    // 5. Serialise to wire format for broadcasting.
    Ok(bincode::serialize(&tx)?)
}
```

Defaults worth noting from `BuilderConfig::default()`:

- `fee_rate = 1000` attonovas/byte
- `min_fee = 10_000` attonovas
- `max_tx_size = 100_000` bytes (quantum signatures are large)
- `dust_threshold = 546` attonovas
- `coin_selection = BranchAndBound` (alternatives: `LargestFirst`,
  `SmallestFirst`, `RandomImprove`)

---

## 7. Language-neutral example (HTTP API)

For integrations that cannot link against the Rust wallet crate, transactions
can be submitted over the HTTP API. The node accepts hex-encoded bincode
blobs at `POST /api/v1/mempool/submit` (or the alias at
`POST /api/v1/blockchain/submit`). Both return the same `txid` on success.

```python
import requests

def broadcast(hex_tx: str, node: str, api_key: str) -> str:
    r = requests.post(
        f"{node}/api/v1/mempool/submit",
        headers={"Authorization": f"Bearer {api_key}"},
        json={"raw_tx": hex_tx},
        timeout=10,
    )
    r.raise_for_status()
    return r.json()["txid"]
```

### Building `hex_tx` from outside Rust

The wire format is bincode-encoded `Transaction`, hex-encoded. For teams
writing a client in another language, the pragmatic path today is:

1. Call the Rust wallet crate behind a thin FFI or gRPC shim, producing the
   signed bincode blob.
2. Pass the hex blob across the language boundary.

There is no hand-rolled transaction-builder reference implementation in
another language at this release. The OpenAPI document at
`docs/api/openapi.json` describes the server-side surface; the wire types
are defined in `supernova-core/src/types/transaction.rs`.

### Authentication and errors

The broadcast endpoint is subject to the full HTTP API policy. See:

- [`api/AUTHENTICATION.md`](api/AUTHENTICATION.md) — the Bearer header and
  the public-endpoint carve-out (note: `POST /api/v1/mempool/submit` is
  **not** on the carve-out; authentication is required).
- [`api/RATE_LIMITING.md`](api/RATE_LIMITING.md) — per-IP sliding-window
  limits and `Retry-After` semantics.
- [`api/ERRORS.md`](api/ERRORS.md) — the stable `code` field to branch on.

A transaction rejected at the mempool layer surfaces as `422
Unprocessable Entity` with a sanitised `message`; correlate via
`request_id` if you need the full error from node logs.

---

## Related

- [`api/README.md`](api/README.md) — full HTTP API index
- [`api/openapi.json`](api/openapi.json) — machine-readable OpenAPI 3.0 spec
- `wallet/src/quantum_wallet/` — the wallet crate
- `supernova-core/src/types/transaction.rs` — wire-format type definitions
- [`NODE_OPERATOR_GUIDE.md`](NODE_OPERATOR_GUIDE.md) — operator-side setup
  for the HTTP endpoint this guide points at
