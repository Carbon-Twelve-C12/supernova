# Fuzzing Supernova

**Status:** production (nightly CI in `.github/workflows/fuzz.yml`).
**Engine:** AFL++ via `cargo-afl`.
**Targets:** 5 — block, transaction, consensus, quantum crypto, quantum P2P.

This document is the single source of truth for running, extending, and
triaging the Supernova fuzz harness. It supersedes the legacy recipe
embedded in `fuzz/README.md`, which retains historical detail on advanced
AFL tuning.

---

## Why fuzzing

Unit tests cover the developer's imagination; property tests cover stated
invariants; fuzzing covers the adversarial long tail. Every entry point
that accepts bytes from the network or disk — block parse, transaction
parse, P2P handshake, signature verify, difficulty adjustment — must be
provably panic-free on any `&[u8]`, including bytes crafted to maximise
parser coverage and minimise code path depth.

Our hard invariants:

- No panic or abort on any input.
- No unbounded allocation (AFL memory limit enforces this).
- No infinite loop (AFL timeout enforces this).
- Any crash or hang is a CI-blocking regression.

---

## Targets

| Binary                       | Public API exercised                       |
| ---------------------------- | ------------------------------------------ |
| `fuzz_block_validation`      | `bincode::deserialize::<Block>`, header    |
|                              | accessors, round-trip serialize            |
| `fuzz_transaction_parsing`   | `bincode::deserialize::<Transaction>`,     |
|                              | input/output/witness walk                  |
| `fuzz_consensus`             | `DifficultyAdjustment::calculate_next_`    |
|                              | `target` with adversarial timestamps       |
| `fuzz_quantum_crypto`        | `MLDSAPublicKey::verify` across all three  |
|                              | NIST security levels                       |
| `fuzz_p2p_messages`          | Quantum-P2P `Handshake` / `Message` /      |
|                              | `PeerInfo` bincode deserialize             |

Each target's first byte (or prefix structure) selects among sub-variants
so that corpus entries can steer the fuzzer without input-length explosion.

---

## Prerequisites

```bash
# Install AFL++ system binaries
sudo apt-get install -y afl++ llvm            # Debian / Ubuntu
brew install afl-fuzz                         # macOS (limited support)

# Install the cargo plugin
cargo install --locked cargo-afl
```

For deep runs, tune the kernel as AFL expects:

```bash
echo core | sudo tee /proc/sys/kernel/core_pattern
sudo sh -c 'echo performance > /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor'
```

---

## Running locally

```bash
cd fuzz

# Build all targets with AFL instrumentation
cargo afl build --release

# Run a single target for 10 minutes against the seed corpus
cargo afl fuzz \
  -i corpus/block_validation \
  -o findings/block_validation \
  -V 600 \
  -- target/release/fuzz_block_validation

# Multi-target loop (unpaced, stops on Ctrl-C)
for target in fuzz_block_validation fuzz_transaction_parsing \
              fuzz_consensus fuzz_quantum_crypto fuzz_p2p_messages; do
  corpus="corpus/$(echo "$target" | sed 's/^fuzz_//')"
  cargo afl fuzz -i "$corpus" -o "findings/$target" -V 300 \
    -- "target/release/$target" &
done
wait
```

A crash lands in `findings/<target>/default/crashes/id:*`. Minimise it:

```bash
cargo afl tmin \
  -i findings/fuzz_block_validation/default/crashes/id:000000,... \
  -o minimized.bin \
  -- target/release/fuzz_block_validation
```

Reproduce deterministically with the minimised input:

```bash
target/release/fuzz_block_validation < minimized.bin
```

---

## Corpus management

The `fuzz/corpus/<target>/` directories are checked into the repo. They
start with trivial seeds (all-zero bytes). Good corpus practice:

- Small is better — AFL mutates faster with < 4 KiB seeds.
- Diverse is better — one input per distinct structural shape.
- Minimise periodically:
  ```bash
  cargo afl cmin \
    -i corpus/block_validation \
    -o corpus/block_validation.min \
    -- target/release/fuzz_block_validation
  mv corpus/block_validation.min corpus/block_validation
  ```

Capturing real-world inputs from a synced node is the gold standard —
see `fuzz/corpus/README.md` for recipes.

---

## Continuous integration

`.github/workflows/fuzz.yml` runs nightly at 03:00 UTC. Each target runs
for 30 minutes by default. The workflow:

1. Installs AFL++ and the compiled binaries for each target.
2. Runs the fuzzer against the in-repo corpus.
3. Uploads every `findings/<target>/` directory as an artifact (30-day
   retention).
4. **Fails the job** if `findings/<target>/default/crashes/` contains any
   `id:*` file.

Re-trigger on demand with `gh workflow run fuzz.yml` or via the GitHub
Actions UI. Override duration with:

```bash
gh workflow run fuzz.yml -f duration_seconds=3600
```

The workflow also fires on any push that changes `fuzz/**` — a target
rewrite that regresses the harness will flag before merge.

---

## Triage workflow

When CI reports a crash:

1. **Download the artifact** — `fuzz-findings-<target>` from the workflow
   run.
2. **Confirm reproducibility** — run the crash input through the local
   binary.
3. **Minimise** — `cargo afl tmin` reduces the input to the smallest
   byte sequence still triggering the crash.
4. **Root-cause** — attach a debugger (`rust-gdb`, `rust-lldb`) or enable
   `RUST_BACKTRACE=full`.
5. **Fix + regression seed** — add the minimised input to the corpus so
   future CI runs prove the regression stays fixed.
6. **File an issue** — link the artifact, minimised input, backtrace, and
   fix PR.

Non-security crashes (e.g., an over-eager `unreachable!()` in a parser
path) are still CI blockers. Panic-safety is a consensus invariant.

---

## Adding a new target

1. Pick an entry point that accepts `&[u8]` or can be driven from one.
2. Write `fuzz/targets/<name>.rs` using the pattern of an existing target:

   ```rust
   use afl::fuzz;
   use supernova_core::<module>::<Type>;

   fn main() {
       fuzz!(|data: &[u8]| {
           let _ = bincode::deserialize::<Type>(data);
       });
   }
   ```

3. Add a `[[bin]]` entry to `fuzz/Cargo.toml`.
4. Create `fuzz/corpus/<name>/` with a trivial seed.
5. Extend the matrix in `.github/workflows/fuzz.yml`.

---

## Related documents

- `docs/security/THREAT_MODEL.md` — classes of bugs fuzzing targets.
- `fuzz/README.md` — historical AFL tuning reference (advanced).
- `docs/operations/INCIDENT_RESPONSE.md` — runbook if a crash indicates
  an exploitable issue on a running network.
