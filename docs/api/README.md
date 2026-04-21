# Supernova HTTP API

This directory is the reference for Supernova's HTTP/JSON API, which a node
exposes for wallets, explorers, mining software, and operational tooling.

**Audience:** integrators, wallet and explorer developers, node operators
writing scripts against a running node.

**Node operators** — the authoritative guide for running and securing a
node is [`../NODE_OPERATOR_GUIDE.md`](../NODE_OPERATOR_GUIDE.md).

---

## At a glance

| Property | Default |
|---|---|
| Bind address | `127.0.0.1` (see below before exposing publicly) |
| Port | `8080` (RPC/JSON), `9000` (Prometheus metrics, separate binary) |
| Base path | `/api/v1/` |
| Versioning | URL prefix (`/api/v1/`, `/api/v2/`, …) |
| Transport | HTTP/1.1 and HTTP/2; TLS termination is the operator's responsibility |
| Serialisation | JSON request and response bodies |
| Authentication | API key in `Authorization: Bearer <key>` header |
| JSON body limit | 4 KiB per request (raw transaction submission excluded via its own path) |
| Rate limit | 100 requests per minute per client IP (configurable) |
| Interactive docs | `/swagger-ui/` on the running node |
| Machine-readable spec | `/api-docs/openapi.json` on the running node |

Never bind the API to a public interface without putting TLS and an
allowlist in front of it. The node's default security posture assumes the
socket is reachable only from trusted addresses.

---

## Contents

### Cross-cutting references

- [`AUTHENTICATION.md`](AUTHENTICATION.md) — API key setup, header formats,
  brute-force protection, and how to expose the admin subset safely.
- [`RATE_LIMITING.md`](RATE_LIMITING.md) — per-client windowing rules, the
  response headers clients should react to, and which routes bypass the
  global limit.
- [`ERRORS.md`](ERRORS.md) — canonical error catalogue (HTTP status, code
  string, message-sanitisation policy).
- [`GENERATING_OPENAPI.md`](GENERATING_OPENAPI.md) — how to regenerate the
  OpenAPI spec from source for SDK generation or offline review.

### Endpoint modules

Each module groups endpoints by subsystem. The list here matches the tags
in the OpenAPI spec.

| Module | Scope | Reference |
|---|---|---|
| `blockchain` | Blocks, transactions, chain info | [`blockchain.md`](blockchain.md) |
| `mempool` | Pending transactions, fee estimates | `mempool` tag in `/swagger-ui/` |
| `network` | Peers, bandwidth, connectivity | `network` tag in `/swagger-ui/` |
| `mining` | Templates, submission, mining controls | `mining` tag in `/swagger-ui/` |
| `environmental` | Energy, carbon, resource metrics | `environmental` tag in `/swagger-ui/` |
| `lightning` | Channels, payments, invoices | `lightning` tag in `/swagger-ui/` |
| `node` | Node management and diagnostics (admin) | [`admin.md`](admin.md) |
| `wallet` | Node-attached wallet operations | [`wallet.md`](wallet.md) |
| `faucet` | Testnet token dispenser | `faucet` tag in `/swagger-ui/` |
| `statistics` | Aggregated stats endpoints | [`statistics.md`](statistics.md) |

Where a per-module markdown file exists it takes precedence over the
Swagger summary for request and response examples; the Swagger view is
always authoritative for the current schema.

---

## Authentication in one command

Authentication is **mandatory** when `api.enable_auth = true` (the
default for any non-local bind). A misconfigured node with `enable_auth`
on but no keys configured refuses to start.

```bash
curl -s \
  -H "Authorization: Bearer $SUPERNOVA_API_KEY" \
  https://node.example.com/api/v1/blockchain/info
```

See [`AUTHENTICATION.md`](AUTHENTICATION.md) for key provisioning, the
"public endpoints" carve-out, and the five-failures-in-five-minutes IP
throttle that protects the auth path.

---

## Rate limiting in one paragraph

Each response carries `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and
`X-RateLimit-Reset`. When the limit is exceeded, the node returns `429
Too Many Requests` with a `Retry-After` header denominated in seconds.
Swagger UI and OpenAPI JSON routes bypass the global limit so that
documentation stays reachable during incidents. Full details in
[`RATE_LIMITING.md`](RATE_LIMITING.md).

---

## Error shape

Every error response is JSON with this shape:

```json
{
  "status": 404,
  "message": "Block not found",
  "code": "NOT_FOUND",
  "request_id": "0e9b3f1c-..."
}
```

Error messages are sanitised server-side to avoid leaking storage
internals. See [`ERRORS.md`](ERRORS.md) for the full code table and the
rules the sanitiser applies.

---

## Example: fetch chain tip info

```bash
curl -s \
  -H "Authorization: Bearer $SUPERNOVA_API_KEY" \
  http://127.0.0.1:8080/api/v1/blockchain/info | jq
```

```json
{
  "chain": "testnet",
  "blocks": 412311,
  "headers": 412311,
  "bestblockhash": "0000000000000000002c1fef...",
  "difficulty": 118742951123.88,
  "mediantime": 1745212345,
  "verificationprogress": 0.999998,
  "pruned": false
}
```

---

## Stability guarantees

- **Path shape** — stable within a major version prefix. A breaking
  change produces a new prefix (`/api/v2/`) and a deprecation window.
- **Field additions** — additive; clients must ignore unknown fields.
- **Field removals and type changes** — require a new version prefix.
- **Error codes** — the `code` string values in [`ERRORS.md`](ERRORS.md)
  are stable; the `message` text is best-effort and may change.

---

## Related

- [`../NODE_OPERATOR_GUIDE.md`](../NODE_OPERATOR_GUIDE.md) — operator-
  facing setup, systemd, firewalling, monitoring.
- [`../RELEASE_PROCESS.md`](../RELEASE_PROCESS.md) — how API-breaking
  changes are versioned and announced.
- [`../security/THREAT_MODEL.md`](../security/THREAT_MODEL.md) — attack
  surface analysis covering the API layer.
