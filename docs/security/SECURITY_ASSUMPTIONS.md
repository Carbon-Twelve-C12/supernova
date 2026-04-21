# Supernova Security Assumptions

**Companion to:** [`THREAT_MODEL.md`](./THREAT_MODEL.md).

This document enumerates the explicit security assumptions that the
Supernova protocol and implementation rely on. Every item is phrased as a
falsifiable claim; violation of any listed assumption invalidates the
corresponding mitigation in the threat model.

---

## A. Adversary model

1. **A-1 Classical compute bound.** The attacker has at most the compute of
   the largest known cloud provider (~10⁹ concurrent cores) for bounded
   time. Out of scope: nation-state-level ASIC farms dedicated solely to
   breaking Supernova.
2. **A-2 Quantum compute.** A fault-tolerant quantum adversary with enough
   logical qubits to run Shor's algorithm on 256-bit ECC in useful time may
   emerge within the economic lifetime of the chain. All long-lived
   cryptographic primitives therefore are post-quantum.
3. **A-3 No oracles on honest keys.** The attacker cannot obtain signatures
   on adversarially chosen messages under honest parties' keys. This is the
   standard EUF-CMA assumption on which ML-DSA security is reduced.
4. **A-4 Byzantine minority.** Up to f < n/3 of any committee (e.g.,
   environmental oracle set) may be Byzantine.
5. **A-5 Hashrate majority.** At any time, > 50 % of global SHA3-512
   hashrate is controlled by economically rational miners who prefer
   long-term chain value over short-term extraction.

## B. Cryptographic primitives

1. **B-1 ML-DSA (Dilithium).** NIST FIPS 204 ML-DSA is EUF-CMA secure at
   the claimed NIST levels. Upstream implementation in `pqcrypto-dilithium`
   is free of timing side channels on supported targets (x86_64, aarch64).
2. **B-2 SPHINCS+.** NIST FIPS 205 SLH-DSA is stateless hash-based and
   EUF-CMA secure given only the hash-function security of SHA3-256 / 512.
3. **B-3 ML-KEM (Kyber).** NIST FIPS 203 ML-KEM-768 is IND-CCA2 secure.
4. **B-4 SHA3-512.** Preimage resistance 2²⁵⁶ post-Grover; collision
   resistance 2¹²⁸ post-quantum.
5. **B-5 BLAKE3.** Used only for internal deduplication / caching, never
   for consensus; assumed collision-resistant at 2¹²⁸ classical.
6. **B-6 Constant-time libraries.** `subtle::ConstantTimeEq` and upstream
   AEAD / signature implementations do not leak secrets through timing.
   Validation of this assumption at binary level is an audit deliverable.
7. **B-7 Domain separation.** All hash invocations that could collide
   across contexts are prefixed with a context tag; collision across tags
   is equivalent to breaking the underlying hash.

## C. Randomness

1. **C-1 OS CSPRNG.** `getrandom()` on Linux ≥ 3.17 (getrandom syscall),
   `BCryptGenRandom` on Windows, and `/dev/urandom` elsewhere produce
   cryptographically strong bits.
2. **C-2 Seed entropy.** BIP-39 mnemonic seeds provide ≥ 128 bits of
   entropy; users do NOT reuse or derive mnemonics from low-entropy
   passphrases.
3. **C-3 No entropy starvation.** A node at steady state always has
   sufficient OS entropy to derive session keys; early-boot entropy
   starvation is mitigated by deferring P2P handshake until the kernel
   signals the pool is initialised.

## D. Network

1. **D-1 Internet connectivity.** Nodes can reach at least one of the
   seed peers listed in `config/*.toml` within 60 s of start-up.
2. **D-2 Peer diversity.** Operators follow the guidance in the operator
   guide to connect to ≥ 8 peers spread across ≥ 3 ASNs.
3. **D-3 Clock sync.** Node clock drift < 2 h relative to median peer
   time. NTP configuration is documented in the operator guide.
4. **D-4 libp2p transport.** libp2p Noise provides authenticated
   encryption for all post-handshake traffic.

## E. Host / operator

1. **E-1 Host integrity.** The machine running the node has not been
   compromised at or below the Rust-userland boundary. A rooted host can
   trivially exfiltrate keys regardless of protocol design.
2. **E-2 Disk encryption.** Operators storing keystores on shared or
   cloud disks use full-disk encryption.
3. **E-3 Process isolation.** Node and wallet run as an unprivileged
   user; operators do not expose the RPC port to the public internet.
4. **E-4 Time source.** Operators run NTP or equivalent; clock drift is
   monitored.
5. **E-5 Binary authenticity.** Operators verify the cosign + GPG
   signatures on release artifacts before installing (documented in the
   operator guide, Phase 4 D1).

## F. Protocol governance

1. **F-1 Soft-fork activation.** Upgrades activate via super-majority
   version-bit signalling; no unilateral change of consensus rules.
2. **F-2 Treasury multisig threshold.** The treasury is a post-quantum
   multisig requiring at least 3-of-5 independent signers drawn from
   diverse organisational and geographic jurisdictions.
3. **F-3 Checkpoint issuance.** Weak-subjectivity checkpoints are
   published with multiple independent signatures; a single compromised
   signer cannot move checkpoint state.

## G. Out-of-scope / non-goals

These are explicitly **not** mitigated at the protocol layer:

1. **G-1 Supply-chain compromise of Rust toolchain** (`rustc`, `cargo`).
   Trusted via Mozilla / Rust Foundation attestations.
2. **G-2 Hardware faults** (Rowhammer, Spectre) that cross userland / OS
   isolation. Mitigated at deployment (ECC RAM, microcode updates).
3. **G-3 Human factors.** Lost passphrases, shoulder-surfing, social
   engineering against operators. Documented in user guidance.
4. **G-4 Anonymous transaction graphs.** Base-layer does not provide
   cryptographic privacy beyond pseudonymity. Planned separately.
5. **G-5 Censorship by state-level adversary** operating above the
   transport layer (BGP, DNS). Mitigation is operational (Tor, alternate
   seed paths) rather than protocol-level.

---

*Violations of any assumption in A/B/C/D/E/F invalidate the mitigations in
the threat model and MUST trigger a protocol-level review.*
