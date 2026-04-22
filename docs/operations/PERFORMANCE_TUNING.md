# Supernova Performance Tuning Guide

## Overview

This guide provides recommendations for optimizing Supernova node performance for different deployment scenarios: high-throughput mining, low-latency API serving, and memory-constrained environments.

---

## Quick Reference

### Common Optimizations

| Scenario | Key Settings | Expected Improvement |
|----------|--------------|---------------------|
| High TPS | Mempool size, signature cache | 2-3x throughput |
| Low Latency | Connection limits, prefetch | 50% latency reduction |
| Memory Limited | Cache sizes, prune mode | 60% memory reduction |

---

## System Configuration

### Operating System Tuning

#### Linux Kernel Parameters

```bash
# /etc/sysctl.d/99-supernova.conf

# Network tuning
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 134217728
net.ipv4.tcp_wmem = 4096 65536 134217728
net.core.netdev_max_backlog = 30000
net.ipv4.tcp_congestion_control = bbr
net.core.default_qdisc = fq

# Connection handling
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535
net.ipv4.tcp_fin_timeout = 10
net.ipv4.tcp_tw_reuse = 1

# File descriptors
fs.file-max = 2097152
fs.nr_open = 2097152

# Memory management
vm.swappiness = 10
vm.dirty_ratio = 60
vm.dirty_background_ratio = 2
```

Apply with:
```bash
sudo sysctl -p /etc/sysctl.d/99-supernova.conf
```

#### File Descriptor Limits

```bash
# /etc/security/limits.d/supernova.conf
supernova soft nofile 1048576
supernova hard nofile 1048576
supernova soft nproc 65536
supernova hard nproc 65536
```

### Disk I/O Optimization

#### Filesystem Mount Options

```bash
# For ext4
/dev/nvme0n1p1 /var/lib/supernova ext4 noatime,nodiratime,discard 0 2

# For XFS
/dev/nvme0n1p1 /var/lib/supernova xfs noatime,nodiratime,logbufs=8 0 2
```

#### I/O Scheduler

```bash
# For NVMe drives
echo none > /sys/block/nvme0n1/queue/scheduler

# For SSDs
echo mq-deadline > /sys/block/sda/queue/scheduler
```

---

## Node Configuration

### Memory Configuration

#### Default Profile (8GB RAM)

```toml
# supernova.toml

[cache]
# UTXO cache: 2GB
utxo_cache_size_mb = 2048

# Signature cache: 512MB, 1M entries
signature_cache_size_mb = 512
signature_cache_entries = 1000000

# Block cache: 256MB
block_cache_size_mb = 256

[mempool]
# Mempool size: 300MB
max_size_mb = 300
max_transactions = 50000
```

#### High Memory Profile (32GB RAM)

```toml
[cache]
utxo_cache_size_mb = 8192
signature_cache_size_mb = 2048
signature_cache_entries = 10000000
block_cache_size_mb = 1024

[mempool]
max_size_mb = 1024
max_transactions = 200000
```

#### Low Memory Profile (4GB RAM)

```toml
[cache]
utxo_cache_size_mb = 512
signature_cache_size_mb = 128
signature_cache_entries = 250000
block_cache_size_mb = 64

[mempool]
max_size_mb = 100
max_transactions = 20000

[storage]
# Enable pruning to save disk
prune_mode = true
prune_target_size_gb = 50
```

### Memory profiling and budgets

The three profiles above express *configured* memory ceilings — what the
node is *allowed* to use. The numbers in this subsection are
*first-principles* budgets for the residual (unconfigured) memory each
role pays on top of those ceilings: transient per-transaction
allocation, per-peer buffers, thread stacks, and allocator overhead.
They are unmeasured today; replace `TBD` rows in
`docs/performance/BASELINE_MEASUREMENTS.md` §2.7 once the profiler has
been run on target hardware.

#### First-principles budget per role

Peak resident-set budgets, assuming the Default (8 GiB) config:

| Role | Configured caches | Transient (budget) | Total budget |
|---|---|---|---|
| Full node, sync | ~3.0 GiB (UTXO 2 GiB + sig 512 MiB + block 256 MiB + mempool 300 MiB) | ~1.5 GiB (peer buffers, decode arenas, rayon stacks) | ~4.5 GiB |
| Miner, block assembly | ~3.5 GiB (Default + mining template cache + larger sig cache) | ~1.0 GiB (template construction, PoW hashing) | ~4.5 GiB |
| Wallet, signing | ~300 MiB (keystore + HD derivation + per-session caches) | ~200 MiB (sig buffers, decoding) | ~500 MiB |

The transient column is where a regression would appear first. Each row
should be validated against a dhat profile before being cited as a
sizing claim.

#### Running the heap profiler

```bash
# Profile the mempool-admission hot path with 10 000 synthetic txs.
cargo run --release --example memory_profile \
    --features dhat-heap -- --tx-count 10000
```

This produces `dhat-heap.json` in the working directory. Open with the
viewer at <https://nnethercote.github.io/dh_view/dh_view.html> and
report the three peak numbers the viewer surfaces — *Total bytes*,
*Total blocks*, *At t-gmax* — into §2.7 of
`BASELINE_MEASUREMENTS.md`. A regression is any increase >10% in *At
t-gmax* for the same `--tx-count`.

For full-node sync and miner block-assembly profiling, run the node/miner
under `valgrind --tool=massif` against a short testnet segment; `dhat`
is not currently wired through the node binary (track E4 follow-up).

#### When to revisit budgets

The budget column above is load-bearing for the Low-memory profile —
the 4 GiB row only works because the transient budget stays under ~1.5
GiB. Re-measure after:

- any change to the transaction validator that adds allocations
  (e.g. a new witness-parse path),
- any change to the P2P framing or decode path,
- any bump in `rayon` parallelism or thread-pool sizing,
- any dependency bump touching `bincode`, `serde`, or a PQ crate.

### Network Configuration

#### Connection Limits

```toml
[network]
# Maximum peer connections
max_inbound_peers = 125
max_outbound_peers = 12

# For high-throughput nodes
max_inbound_peers = 250
max_outbound_peers = 24

# For low-bandwidth environments
max_inbound_peers = 50
max_outbound_peers = 8
```

#### Block Propagation

```toml
[network]
# Compact block relay
compact_blocks_enabled = true
compact_block_prefill_txs = 10

# Block announcement
block_announcement_mode = "header_first"

# Bandwidth limits
max_upload_rate_mbps = 100
max_download_rate_mbps = 100
```

### Database Configuration

#### RocksDB Tuning

```toml
[database]
# Write buffer
write_buffer_size_mb = 128
max_write_buffer_number = 4

# Block cache (separate from UTXO cache)
block_cache_size_mb = 512

# Compaction
max_background_compactions = 4
max_background_flushes = 2

# Compression
compression = "lz4"
bottommost_compression = "zstd"
```

#### High IOPS Configuration

```toml
[database]
write_buffer_size_mb = 256
max_write_buffer_number = 6
block_cache_size_mb = 1024
max_background_compactions = 8
max_background_flushes = 4
allow_concurrent_memtable_write = true
```

---

## Scenario-Specific Tuning

### Mining Node

Optimized for block production:

```toml
[mining]
# Enable mining
enabled = true

# Block template caching
template_cache_size = 5
template_refresh_ms = 500

# Transaction selection
max_block_weight = 4000000
prioritize_fee_rate = true

# Getblocktemplate optimization
gbt_cache_ms = 100

[cache]
# Large signature cache for fast validation
signature_cache_entries = 5000000
signature_cache_size_mb = 1024

[mempool]
# Large mempool for transaction selection
max_size_mb = 500
max_transactions = 100000
# Faster eviction of low-fee txs
min_fee_rate = 1.0
```

### API Server Node

Optimized for RPC performance:

```toml
[api]
# HTTP server
max_connections = 1000
request_timeout_ms = 30000
worker_threads = 8

# Response caching
enable_response_cache = true
response_cache_size_mb = 256
response_cache_ttl_seconds = 5

# Rate limiting
rate_limit_requests_per_second = 100
rate_limit_burst = 200

[cache]
# Larger block cache for block queries
block_cache_size_mb = 1024

# TX index for lookups
txindex = true
```

### Archive Node

Full historical data:

```toml
[storage]
# No pruning
prune_mode = false

# Full indexes
txindex = true
addressindex = true
timestampindex = true

[database]
# Optimized for reads
block_cache_size_mb = 2048
max_open_files = 5000
```

### Lightweight Node (Pruned)

Minimal disk usage:

```toml
[storage]
# Aggressive pruning
prune_mode = true
prune_target_size_gb = 10

# No extra indexes
txindex = false
addressindex = false

[cache]
# Smaller caches
utxo_cache_size_mb = 256
block_cache_size_mb = 64
```

---

## Performance Monitoring

### Key Metrics to Monitor

```bash
# CPU usage per component
supernova-cli getprocessinfo | jq '.cpu_usage'

# Memory breakdown
supernova-cli getmemoryinfo | jq '.'

# Cache hit rates
supernova-cli getcacheinfo | jq '.hit_rate'

# Database performance
supernova-cli getdbinfo | jq '.read_latency_ms, .write_latency_ms'
```

### Prometheus Metrics

```yaml
# Important metrics to track

# Cache effectiveness
supernova_cache_hits_total
supernova_cache_misses_total

# Database latency
supernova_db_read_seconds_bucket
supernova_db_write_seconds_bucket

# Memory usage
supernova_memory_usage_bytes{component="utxo_cache"}
supernova_memory_usage_bytes{component="mempool"}

# Network throughput
supernova_bytes_sent_total
supernova_bytes_received_total
```

### Benchmark Commands

```bash
# Block validation benchmark
supernova-cli benchmark validate-blocks --count 1000

# Signature verification benchmark
supernova-cli benchmark verify-signatures --count 10000

# Database throughput
supernova-cli benchmark db-iops

# Network throughput
supernova-cli benchmark network-throughput
```

---

## Common Performance Issues

### High CPU Usage

**Symptoms:** CPU at 100%, slow block validation

**Solutions:**
1. Increase signature cache:
   ```toml
   signature_cache_entries = 5000000
   ```
2. Enable parallel validation:
   ```toml
   parallel_validation_threads = 8
   ```
3. Check for revalidation loops in logs

### High Memory Usage

**Symptoms:** Memory growing unbounded, OOM kills

**Solutions:**
1. Reduce cache sizes:
   ```toml
   utxo_cache_size_mb = 1024
   mempool.max_size_mb = 200
   ```
2. Enable memory limits:
   ```toml
   max_memory_mb = 8192
   ```
3. Check for memory leaks with:
   ```bash
   supernova-cli getmemoryinfo --detailed
   ```

### High Disk I/O

**Symptoms:** Slow sync, high iowait

**Solutions:**
1. Increase write buffers:
   ```toml
   write_buffer_size_mb = 256
   max_write_buffer_number = 4
   ```
2. Enable compression:
   ```toml
   compression = "lz4"
   ```
3. Upgrade to NVMe storage

### Network Bottlenecks

**Symptoms:** Slow block propagation, peer timeouts

**Solutions:**
1. Increase connection limits:
   ```toml
   max_outbound_peers = 24
   ```
2. Enable compact blocks:
   ```toml
   compact_blocks_enabled = true
   ```
3. Check bandwidth limits:
   ```toml
   max_upload_rate_mbps = 0  # unlimited
   ```

---

## Hardware Recommendations

### Minimum (Pruned Node)

- CPU: 4 cores, 2.5 GHz
- RAM: 4 GB
- Storage: 100 GB SSD
- Network: 10 Mbps

### Recommended (Full Node)

- CPU: 8 cores, 3.0 GHz
- RAM: 16 GB
- Storage: 1 TB NVMe SSD
- Network: 100 Mbps

### High Performance (Mining/API)

- CPU: 16+ cores, 3.5 GHz
- RAM: 32-64 GB
- Storage: 2 TB NVMe SSD (PCIe 4.0)
- Network: 1 Gbps

### Archive Node

- CPU: 8+ cores
- RAM: 32 GB
- Storage: 4+ TB SSD
- Network: 100 Mbps

---

## Tuning Checklist

- [ ] OS kernel parameters configured
- [ ] File descriptor limits increased
- [ ] Disk I/O optimized for SSD/NVMe
- [ ] Memory allocation matches available RAM
- [ ] Cache sizes tuned for workload
- [ ] Network limits configured
- [ ] Database tuned for read/write ratio
- [ ] Monitoring in place for key metrics
- [ ] Alerts configured for resource exhaustion
