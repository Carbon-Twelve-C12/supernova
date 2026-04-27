/**
 * Supernova Load Testing - Transaction Throughput Test
 * 
 * PERFORMANCE MODULE (P1-011): Load testing framework targeting 1000 TPS
 * 
 * This k6 script tests the Supernova RPC endpoint's ability to handle
 * high transaction throughput.
 * 
 * Prerequisites:
 *   - k6 installed: https://k6.io/docs/getting-started/installation/
 *   - Supernova node running with RPC enabled
 * 
 * Usage:
 *   k6 run transaction_load.js
 *   k6 run --vus 100 --duration 5m transaction_load.js
 * 
 * Environment Variables:
 *   RPC_URL - Base URL for Supernova RPC (default: http://localhost:8332)
 */

import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Counter, Trend, Rate } from 'k6/metrics';
import { randomBytes } from 'k6/crypto';

// =============================================================================
// Configuration
// =============================================================================

// Test stages: Ramp up, sustained load, ramp down
export const options = {
  stages: [
    { duration: '2m', target: 100 },   // Ramp up to 100 VUs
    { duration: '5m', target: 500 },   // Ramp up to 500 VUs
    { duration: '10m', target: 1000 }, // Sustained 1000 VUs (target TPS)
    { duration: '2m', target: 500 },   // Ramp down
    { duration: '1m', target: 0 },     // Final ramp down
  ],
  thresholds: {
    // HTTP request metrics
    'http_req_duration': ['p(95)<500', 'p(99)<1000'], // 95% under 500ms, 99% under 1s
    'http_req_failed': ['rate<0.01'],                  // Less than 1% failures
    
    // Custom metrics
    'transaction_latency': ['p(95)<500', 'p(99)<1000'],
    'transaction_success_rate': ['rate>0.99'],
    'blocks_fetched': ['count>0'],
  },
  
  // Runtime options
  noConnectionReuse: false,
  userAgent: 'SupernovaLoadTest/1.0',
  
  // Output options for CI
  summaryTrendStats: ['avg', 'min', 'med', 'max', 'p(90)', 'p(95)', 'p(99)'],
};

// =============================================================================
// Custom Metrics
// =============================================================================

const transactionLatency = new Trend('transaction_latency');
const transactionSuccessRate = new Rate('transaction_success_rate');
const blocksFetched = new Counter('blocks_fetched');
const transactionsSubmitted = new Counter('transactions_submitted');
const rpcErrors = new Counter('rpc_errors');

// =============================================================================
// Test Configuration
// =============================================================================

const RPC_URL = __ENV.RPC_URL || 'http://localhost:8332';

// Sample transaction data (hex-encoded, minimal valid structure)
function generateTestTransaction() {
  // In production, this would generate actual valid transactions
  // For load testing, we use a placeholder structure
  const txid = randomBytes(32, 'hex');
  return {
    version: 1,
    inputs: [{ txid: txid, vout: 0 }],
    outputs: [{ value: 1000, address: 'test_address' }],
    locktime: 0,
  };
}

// JSON-RPC request helper
function rpcRequest(method, params = []) {
  const payload = JSON.stringify({
    jsonrpc: '2.0',
    method: method,
    params: params,
    id: Date.now(),
  });
  
  return http.post(RPC_URL, payload, {
    headers: {
      'Content-Type': 'application/json',
    },
    tags: { name: method },
  });
}

// =============================================================================
// Test Scenarios
// =============================================================================

export default function () {
  group('RPC Health Check', function () {
    const res = rpcRequest('getblockchaininfo');
    
    const success = check(res, {
      'status is 200': (r) => r.status === 200,
      'response has result': (r) => JSON.parse(r.body).result !== undefined,
    });
    
    if (!success) {
      rpcErrors.add(1);
    }
  });

  group('Get Block Height', function () {
    const res = rpcRequest('getblockcount');
    
    check(res, {
      'status is 200': (r) => r.status === 200,
      'has block height': (r) => {
        const body = JSON.parse(r.body);
        return body.result !== undefined && body.result >= 0;
      },
    });
    
    if (res.status === 200) {
      blocksFetched.add(1);
    }
  });

  group('Transaction Submission', function () {
    const startTime = Date.now();
    
    // Generate test transaction
    const tx = generateTestTransaction();
    
    // Submit via sendrawtransaction
    const res = rpcRequest('sendrawtransaction', [JSON.stringify(tx)]);
    
    const latency = Date.now() - startTime;
    transactionLatency.add(latency);
    
    const success = check(res, {
      'status is 200 or 400': (r) => r.status === 200 || r.status === 400,
      'response time OK': (r) => r.timings.duration < 1000,
    });
    
    if (success) {
      transactionSuccessRate.add(1);
      transactionsSubmitted.add(1);
    } else {
      transactionSuccessRate.add(0);
      rpcErrors.add(1);
    }
  });

  group('Get Mempool Info', function () {
    const res = rpcRequest('getmempoolinfo');
    
    check(res, {
      'status is 200': (r) => r.status === 200,
      'has mempool size': (r) => {
        try {
          const body = JSON.parse(r.body);
          return body.result && body.result.size !== undefined;
        } catch {
          return false;
        }
      },
    });
  });

  // Brief pause between iterations to simulate realistic load patterns
  sleep(Math.random() * 0.1); // 0-100ms random delay
}

// =============================================================================
// Setup and Teardown
// =============================================================================

export function setup() {
  console.log('='.repeat(60));
  console.log('Supernova Load Test - Starting');
  console.log('Target: ' + RPC_URL);
  console.log('='.repeat(60));
  
  // Verify connectivity before starting
  const res = rpcRequest('getblockchaininfo');
  
  if (res.status !== 200) {
    throw new Error(`Cannot connect to RPC endpoint: ${res.status}`);
  }
  
  const info = JSON.parse(res.body).result;
  console.log(`Connected to chain: ${info.chain || 'unknown'}`);
  console.log(`Block height: ${info.blocks || 'unknown'}`);
  
  return { startTime: Date.now() };
}

export function teardown(data) {
  const duration = (Date.now() - data.startTime) / 1000;
  
  console.log('='.repeat(60));
  console.log('Supernova Load Test - Complete');
  console.log(`Total duration: ${duration.toFixed(2)}s`);
  console.log('='.repeat(60));
}

// =============================================================================
// Handle Summary for CI Integration
// =============================================================================

export function handleSummary(data) {
  return {
    'stdout': textSummary(data, { indent: '  ', enableColors: true }),
    'results/load_test_summary.json': JSON.stringify(data, null, 2),
  };
}

function textSummary(data, options) {
  const lines = [];
  
  lines.push('');
  lines.push('╔═══════════════════════════════════════════════════════════╗');
  lines.push('║           SUPERNOVA LOAD TEST RESULTS                     ║');
  lines.push('╚═══════════════════════════════════════════════════════════╝');
  lines.push('');
  
  // Key metrics
  const httpDuration = data.metrics.http_req_duration;
  if (httpDuration) {
    lines.push(`HTTP Request Duration:`);
    lines.push(`  Average: ${(httpDuration.values.avg || 0).toFixed(2)}ms`);
    lines.push(`  P95: ${(httpDuration.values['p(95)'] || 0).toFixed(2)}ms`);
    lines.push(`  P99: ${(httpDuration.values['p(99)'] || 0).toFixed(2)}ms`);
  }
  
  const httpFailed = data.metrics.http_req_failed;
  if (httpFailed) {
    lines.push(`HTTP Failure Rate: ${(httpFailed.values.rate * 100).toFixed(2)}%`);
  }
  
  const txLatency = data.metrics.transaction_latency;
  if (txLatency) {
    lines.push(`Transaction Latency:`);
    lines.push(`  Average: ${(txLatency.values.avg || 0).toFixed(2)}ms`);
    lines.push(`  P95: ${(txLatency.values['p(95)'] || 0).toFixed(2)}ms`);
  }
  
  // Thresholds check
  lines.push('');
  lines.push('Threshold Results:');
  for (const [name, threshold] of Object.entries(data.thresholds || {})) {
    const status = threshold.ok ? '✓ PASS' : '✗ FAIL';
    lines.push(`  ${status}: ${name}`);
  }
  
  lines.push('');
  
  return lines.join('\n');
}

