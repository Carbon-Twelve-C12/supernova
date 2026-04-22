# Chaos testing strategy

This document defines Supernova's chaos-testing surface: which fault
classes are covered, how they are exercised in-tree today, and what is
deferred to the multi-node testnet track. It is the counterpart of
`docs/performance/BASELINE_MEASUREMENTS.md` §2.8 — the performance
document records measured numbers, this one records the experimental
plan behind them.

The goal of chaos testing is **not** to hit a performance target. It
is to make sure the properties the node promises under adversarial
conditions — liveness, safety, convergence — are not silently lost
when a commit changes timing or control flow.

---

## 1. Invariants under test

A chaos run is considered passing when **all** of the following hold at
the end of the scenario, after the last injected fault has been healed:

1. **Safety.** No two non-failed nodes expose different tip hashes at
   the same height at `t_end`.
2. **Liveness.** Each running node's tip advanced at least once during
   the scenario.
3. **No silent fork retention.** The set of finalised blocks is a
   prefix of the tip chain on every node. No node is holding a
   stale-tip view below the k-deep reorg horizon.
4. **Bounded reorg depth.** Any observed reorg is ≤ `MAX_REORG_DEPTH`
   (weak subjectivity bound, consensus module).
5. **Zero consensus-protocol panics.** The runtime must not have
   propagated any `panic!` / `unwrap` originating from consensus,
   validation, or networking modules. This is checked via the panic
   hook in the test harness.

Any violation is a release-blocker, not a flake. Chaos tests that
flake are chaos tests with an unpinned non-determinism source — fix
the source, not the test.

---

## 2. Fault classes

Four classes of fault are in scope. Each maps to an existing in-tree
primitive; the right-hand column says whether the primitive is
callable from a unit-level in-tree test today or only from the
multi-node testnet harness that track E1 will stand up.

| Class | Plan name | Primitive | In-tree today? |
|---|---|---|---|
| Crash | node failure + restart | `TestScenario` step — drop the node from the simulator, later re-add | Yes — `test_harness.rs` supports arbitrary step sequences |
| Partition | network partition | `TestStep::CreatePartition` / `HealPartition`; `NetworkCondition { is_severed: true }` | Yes — `example_scenarios::network_partition_scenario()` |
| Clock drift | node clock skew | `NetworkSimulator::clock_drift_ms` (per-node offset, signed ms) | Yes — `SimulationConfig::simulate_clock_drift`; consensus `TimeManager` handles MTP |
| Byzantine oracle | environmental oracle misreports | Oracle registry quorum + slashing — `supernova-core/src/environmental/oracle_registry.rs` | Unit-level yes (oracle module); sustained multi-node run deferred |

Signature-downgrade and double-spend are separately covered by the
consensus and crypto test suites and are not repeated here.

---

## 3. Scenarios

Each scenario is a named sequence of `TestStep`s, an expected
`TestOutcome` set, and a fault budget. Where the scenario is already
implemented, the right column is a file/function reference. Where it is
not, the scenario is deferred to the multi-node testnet (track E1 of
Phase 5 follow-up).

### 3.1 Partition-and-heal

- **Steps:** mine N common blocks → split into two groups → each
  group mines independently → heal → observe convergence.
- **Assertion:** all nodes have the same tip at `t_end`; the orphaned
  branch is reorged within one block interval.
- **Status:** implemented.
- **Reference:** `supernova-core/src/testnet/test_harness.rs::example_scenarios::network_partition_scenario`.

### 3.2 Crash-and-restart under load

- **Steps:** warm the mempool with ~10 k synthetic transactions → kill
  the leader miner → wait one block interval → restart → observe
  mempool replenishment and chain continuation from a non-restarted
  miner.
- **Assertion:** no orphan beyond depth 2; restarted node's tip
  matches the rest within 30 s.
- **Status:** primitive available (harness supports adding / removing
  nodes), full scenario not yet checked-in. One-hour author-cost
  follow-up.

### 3.3 Clock-drift skew

- **Steps:** configure 5-node topology; node 0 skewed +7 minutes, node
  1 skewed −7 minutes, rest at t=0 → mine across the boundary of the
  median-time-past window → submit a block whose header timestamp is
  outside the acceptable range on the skewed nodes.
- **Assertion:** every correctly-clocked node accepts the canonical
  chain; skewed nodes either reject the offending block locally
  (consensus `TimeManager` rejects `ts > now + 2h`) or self-correct
  after MTP re-anchors.
- **Status:** primitive available; scenario expressible via
  `SimulationConfig::simulate_clock_drift` and per-node
  `NetworkSimulator::clock_drift_ms`. Not yet wrapped as a
  `TestScenario`.

### 3.4 Byzantine oracle

- **Steps:** stand up the environmental oracle registry with n=4
  participants; one signs contradictory readings for the same window.
- **Assertion:** the contradictory reading is rejected by the quorum,
  the offending oracle is slashed, block validation is unaffected by
  the rejected attestation.
- **Status:** unit-level tests exist for the quorum logic
  (`supernova-core/src/environmental/oracle_registry.rs`). Running it
  against a live 10-node testnet for 24 hours with injected Byzantine
  behaviour is deferred to track E1 follow-up.

### 3.5 (Deferred) 24-hour mixed chaos soak

- **Steps:** 10-node testnet with cross-region latency profile (0–200
  ms); inject a random fault every 5 minutes drawn uniformly from
  §3.1–§3.4; run for 24 wall-clock hours.
- **Assertion:** §1 invariants hold for every minute of the run; the
  orphan rate is < 0.1 % of produced blocks.
- **Status:** explicitly deferred. Requires the multi-node testnet
  harness from track E1 (not in-tree at commit-time of this document).
  Recorded here so the invariant list is reused verbatim when the
  harness lands.

---

## 4. Running the in-tree scenarios

The scenarios in §3.1 are currently accessible via the `TestNetManager`
+ `test_harness::run_scenario` path. Example invocation from an
in-process test:

```rust
use supernova_core::testnet::{TestNetManager, TestNetConfig};
use supernova_core::testnet::test_harness::example_scenarios::network_partition_scenario;

let mut manager = TestNetManager::new(TestNetConfig::default());
let scenario = network_partition_scenario();
let result = manager.run_scenario(scenario).await;
assert!(result.is_success());
```

§3.2 / §3.3 / §3.4 should follow the same pattern — build the scenario
from `TestStep` variants, define `expected_outcomes`, feed to
`run_scenario`. When they land, add them to
`supernova-core/src/testnet/test_harness.rs::example_scenarios`.

---

## 5. Known gaps and cleanup

- `node/src/tests/chaos_testing.rs`, `clock_drift_tests.rs`,
  `network_partition_tests.rs`, `large_block_tests.rs`, and
  `fork_handling.rs` are orphaned — they reference
  `crate::network::{NetworkSimulator, NodeHandle, NetworkCondition,
  NodeConfig}` paths that do not exist in the current architecture.
  The canonical simulator lives in
  `supernova-core/src/testnet/network_simulator.rs` instead. The
  `node/src/tests/mod.rs` file is not imported from `node/src/lib.rs`,
  so these files contribute zero to the test run today. They should
  either be rewritten against the real testnet API or deleted; that
  cleanup is out of scope for track E5 and is tracked as a separate
  follow-up.
- The 24-hour multi-node soak in §3.5 has no owner in this repo at
  commit time; once the multi-node testnet is stood up, §3.5 becomes
  the acceptance gate for it.

---

## 6. Related

- `supernova-core/src/testnet/test_harness.rs` — scenario primitives
- `supernova-core/src/testnet/network_simulator.rs` — fault-injection
  surface (partition / latency / packet loss / clock drift)
- `docs/performance/BASELINE_MEASUREMENTS.md` §2.8 — measured outcomes
  once the scenarios are run
- `docs/testing/TESTING_STRATEGY.md` — broader test-strategy context
