# Supernova Load Testing Framework

## Overview

This directory contains load testing scripts for validating Supernova's performance under high traffic conditions.

**Target Metrics:**
- 1000 TPS sustained throughput
- P95 latency < 500ms
- < 1% error rate under load
- 10,000 concurrent RPC connections

## Prerequisites

### Install k6

```bash
# macOS
brew install k6

# Linux (Debian/Ubuntu)
sudo gpg -k
sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt-get update
sudo apt-get install k6

# Docker
docker pull grafana/k6
```

## Test Scripts

### `transaction_load.js` - Transaction Throughput Test

Tests the RPC endpoint's ability to handle high transaction volumes.

```bash
# Basic run
k6 run transaction_load.js

# Custom RPC endpoint
RPC_URL=http://your-node:8332 k6 run transaction_load.js

# Override VUs and duration
k6 run --vus 100 --duration 5m transaction_load.js

# Output results to InfluxDB (for Grafana visualization)
k6 run --out influxdb=http://localhost:8086/k6 transaction_load.js
```

### `concurrent_connections.js` - Connection Test

Tests handling of many concurrent RPC connections.

```bash
k6 run concurrent_connections.js
```

## Performance Baselines

These are the minimum acceptable metrics for production:

| Metric | Threshold | Notes |
|--------|-----------|-------|
| TPS | ≥1000 | Sustained for 1 hour |
| P50 Latency | <100ms | Median response time |
| P95 Latency | <500ms | 95th percentile |
| P99 Latency | <1000ms | 99th percentile |
| Error Rate | <1% | HTTP and RPC errors |
| Concurrent Connections | 10,000 | Simultaneous |

## Test Scenarios

### 1. Transaction Load Test

**Purpose:** Validate maximum sustained TPS

**Stages:**
1. Ramp up: 2 minutes to 100 VUs
2. Ramp up: 5 minutes to 500 VUs  
3. Sustained: 10 minutes at 1000 VUs
4. Ramp down: 3 minutes

**Success Criteria:**
- [ ] P95 latency < 500ms
- [ ] Error rate < 1%
- [ ] No crashes or OOM events
- [ ] Mempool size stays manageable

### 2. Block Propagation Under Load

**Purpose:** Ensure blocks propagate quickly during high traffic

**Stages:**
1. Generate sustained transaction load
2. Mine/produce blocks
3. Measure propagation time

**Success Criteria:**
- [ ] Block propagation < 2 seconds at P95
- [ ] All nodes converge to same tip

### 3. Mempool Flooding

**Purpose:** Test mempool limits and rejection handling

**Stages:**
1. Submit 100,000+ transactions rapidly
2. Verify rate limiting activates
3. Verify node remains responsive

**Success Criteria:**
- [ ] Node doesn't crash
- [ ] Memory usage stays bounded
- [ ] Legitimate requests still processed

### 4. Large Block Processing

**Purpose:** Validate max block size handling

**Stages:**
1. Create block at maximum size
2. Propagate to all nodes
3. Measure processing time

**Success Criteria:**
- [ ] Processing < 1 second
- [ ] No timeouts during validation

## CI Integration

Add to your CI pipeline:

```yaml
# .github/workflows/load-test.yml
name: Load Test

on:
  schedule:
    - cron: '0 2 * * 0'  # Weekly on Sunday at 2 AM
  workflow_dispatch:

jobs:
  load-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Start Supernova node
        run: |
          docker-compose -f deployment/testnet/docker-compose.yml up -d
          sleep 30
          
      - name: Install k6
        run: |
          sudo gpg -k
          sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491D6AC1D69
          echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
          sudo apt-get update
          sudo apt-get install k6
          
      - name: Run load tests
        run: |
          mkdir -p results
          k6 run tests/load/transaction_load.js
          
      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: load-test-results
          path: results/
```

## Interpreting Results

### Sample Output

```
╔═══════════════════════════════════════════════════════════╗
║           SUPERNOVA LOAD TEST RESULTS                     ║
╚═══════════════════════════════════════════════════════════╝

HTTP Request Duration:
  Average: 45.23ms
  P95: 234.56ms
  P99: 456.78ms
HTTP Failure Rate: 0.12%
Transaction Latency:
  Average: 67.89ms
  P95: 345.67ms

Threshold Results:
  ✓ PASS: http_req_duration
  ✓ PASS: http_req_failed
  ✓ PASS: transaction_latency
  ✓ PASS: transaction_success_rate
```

### What to Look For

1. **P95/P99 Latency Spikes**
   - May indicate garbage collection pauses
   - Could be resource contention
   - Check node logs during spikes

2. **Error Rate Increases**
   - Rate limiting kicking in (expected at high load)
   - Actual errors (check logs)
   - Network issues

3. **Memory Growth**
   - Monitor node memory during test
   - Should stabilize, not grow indefinitely

## Troubleshooting

### High Latency

```bash
# Check node resource usage
docker stats supernova-node

# Check database performance
du -sh ~/.supernova/data/

# Check mempool size via RPC
curl -s -X POST http://localhost:8332 -d '{"jsonrpc":"2.0","method":"getmempoolinfo","id":1}'
```

### Connection Errors

```bash
# Check open file limits
ulimit -n

# Increase if needed
ulimit -n 65535

# Check TCP settings
sysctl net.ipv4.tcp_max_syn_backlog
```

### Memory Issues

```bash
# Monitor during test
watch -n 1 'free -h'

# Check for leaks
valgrind --leak-check=full ./supernova-node
```

## Performance Report Template

After each performance test run, document results in `PERFORMANCE.md`:

```markdown
# Performance Report - YYYY-MM-DD

## Environment
- Node version: X.Y.Z
- Hardware: [specs]
- Network: [configuration]

## Results

| Metric | Value | Threshold | Status |
|--------|-------|-----------|--------|
| Max TPS | 1234 | 1000 | ✓ |
| P95 Latency | 234ms | 500ms | ✓ |
| P99 Latency | 456ms | 1000ms | ✓ |
| Error Rate | 0.5% | 1% | ✓ |

## Observations
- [Any notable observations]

## Action Items
- [ ] [Any improvements needed]
```

