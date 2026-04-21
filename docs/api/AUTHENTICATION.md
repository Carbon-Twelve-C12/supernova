# Authentication

Supernova's HTTP API authenticates clients with a long-lived API key sent
in the `Authorization` header. Authentication is **mandatory** for every
route except a small, explicitly-enumerated public list.

This page covers:

1. Configuring keys on the node
2. Sending authenticated requests
3. The public-endpoint carve-out
4. Brute-force protection on the auth path
5. Hardening the admin subset of the API

Operator-side setup (TLS termination, binding, systemd) lives in
[`../NODE_OPERATOR_GUIDE.md`](../NODE_OPERATOR_GUIDE.md).

---

## 1. Configuring keys on the node

Keys are configured under `[api]` in the node configuration:

```toml
[api]
enable_auth = true
api_keys = [
    "op-dashboard-7f4d…",   # operator dashboard
    "indexer-sidecar-a3e2…", # per-service keys, never shared
]
bind_address = "127.0.0.1"
port = 8080
rate_limit = 100
```

### Generating a key

Any high-entropy random string works. A 32-byte base64-encoded value is
a reasonable default:

```bash
openssl rand -base64 32
# → vYG9Qk6HkLmH1WkVLm5fZB2n6X1Q1y5p0TsO3s4X8jA=
```

### Rules the node enforces

The node refuses to start with insecure configurations. It will reject:

- `enable_auth = true` with an empty `api_keys` list. This produces
  an explicit `SECURITY ERROR: Authentication enabled but no API keys
  configured` and exits.
- Any entry in `api_keys` that is empty or whitespace-only.

It will log a warning but still start for:

- `enable_auth = false` (suitable only for localhost-only benches).
- Any key containing the substring `CHANGE-ME`, which flags that a
  template placeholder was not replaced.

Rotate keys by adding the new key, restarting, updating every client,
then restarting again with the old key removed.

---

## 2. Sending authenticated requests

Attach the key in the `Authorization` header. Both canonical `Bearer`
form and the bare key are accepted:

```bash
# Preferred — matches the OpenAPI spec's security scheme:
curl -H "Authorization: Bearer $SUPERNOVA_API_KEY" \
  https://node.example.com/api/v1/node/status

# Equivalent (legacy form):
curl -H "Authorization: $SUPERNOVA_API_KEY" \
  https://node.example.com/api/v1/node/status
```

Failed authentication returns `401 Unauthorized`:

```json
{
  "error": "Authentication required",
  "message": "Please provide a valid API key in the Authorization header",
  "code": "AUTH_REQUIRED"
}
```

A request without the header, with an unrecognised key, or with a
malformed header value all return the same response. That uniformity is
deliberate — distinguishing them would leak information about key
validity.

---

## 3. The public-endpoint carve-out

A short, explicit list of endpoints bypasses authentication so that
health checks and faucet workflows remain accessible:

| Endpoint | Why it bypasses auth |
|---|---|
| `GET /health` | Used by load balancers and orchestrators for liveness checks |
| `GET /api/v1/blockchain/info` | Read-only chain tip; surfaces no secrets |
| `GET /api/v1/blockchain/height` | Read-only chain height |
| `POST /api/v1/faucet/request` | Testnet faucet; protected instead by its own per-IP/address rate limit and CAPTCHA |

Adding to this list requires a change to
`node/src/api/middleware/mandatory_auth.rs` and re-review of the
threat model. Do not open up routes without that process.

---

## 4. Brute-force protection on the auth path

The `Authorization` header is the highest-value input on the API
surface, so the node applies a second rate-limiter specifically to
authentication attempts, independent of the global request limit.

Default policy (`AuthRateLimiterConfig` in
`node/src/api/middleware/auth_rate_limiter.rs`):

| Parameter | Default | Behaviour |
|---|---|---|
| `max_failed_attempts` | `5` | After five failed attempts… |
| `attempt_window_secs` | `300` | …within a five-minute window… |
| `block_duration_secs` | `3600` | …the offending IP is blocked for one hour. |
| `max_attempts_per_minute` | `10` | Even below the failure threshold, an IP cannot exceed ten auth attempts per minute. |

When an IP is blocked, every request — including previously-valid
requests from that IP — returns `403 Forbidden` until the block window
elapses. Successful authentications reset the failure counter for the
source IP.

This layer sits **before** the global request rate limiter, so it
protects the keyed path even when the global limit has not been
reached.

---

## 5. Hardening the admin subset

Endpoints that mutate node state or expose operational internals live
under the `node` tag. These deserve stricter treatment than read-only
paths.

### What is considered admin

| Method + path | Effect |
|---|---|
| `GET /api/v1/node/config` | Dump runtime config (may include partial secrets redacted) |
| `PUT /api/v1/node/config` | Mutate runtime config |
| `POST /api/v1/node/restart` | Restart the node process |
| `POST /api/v1/node/shutdown` | Terminate the node process |
| `GET /api/v1/node/logs` | Retrieve recent structured logs |
| `POST /api/v1/node/backup` | Trigger a backup |
| `GET /api/v1/node/metrics` | Node-internal metrics (distinct from Prometheus) |
| `GET /api/v1/node/debug` | Internal diagnostics |

### Recommended hardening

1. **Separate keys per role.** Mint one key per client and scope it to
   what that client actually needs. Do not share the same key between
   mining software, block explorers, and operational dashboards.
2. **Bind to a management interface.** Expose the API on a dedicated
   interface (for example, `10.0.0.0/8`) rather than `0.0.0.0`. Put
   a firewall rule in front that only admits known client addresses.
3. **Never expose admin endpoints to the public internet.** If your
   node serves a public block explorer, reverse-proxy `/api/v1/blockchain/*`
   and `/api/v1/statistics/*` but deny every other subtree at the proxy.
4. **Terminate TLS.** An API key travels in every request; without TLS
   it is harvested at the first network hop. Use a reverse proxy
   (nginx, Caddy, Envoy) or Cloudflare to terminate TLS in front of the
   node socket.
5. **Log audit events centrally.** The node emits a log line on every
   unauthorised access attempt and every admin mutation; forward those
   to your SIEM.

### Example nginx guard

```nginx
server {
    listen 443 ssl http2;
    server_name node.example.com;

    # Public read-only subset — no auth required.
    location ~ ^/api/v1/blockchain/(info|height)$ {
        proxy_pass http://127.0.0.1:8080;
    }

    # Admin subset — restrict to operator CIDR.
    location ~ ^/api/v1/node/ {
        allow 10.0.0.0/8;
        deny all;
        proxy_pass http://127.0.0.1:8080;
    }

    # Everything else — authenticated clients only.
    location /api/v1/ {
        proxy_set_header Authorization $http_authorization;
        proxy_pass http://127.0.0.1:8080;
    }
}
```

---

## Related

- [`RATE_LIMITING.md`](RATE_LIMITING.md) — the global request rate
  limiter that runs after this layer.
- [`ERRORS.md`](ERRORS.md) — response shape for `401`, `403`, and
  `429`.
- [`../NODE_OPERATOR_GUIDE.md`](../NODE_OPERATOR_GUIDE.md) §3 — where to
  place `api_keys` in the systemd-managed config.
- [`../security/THREAT_MODEL.md`](../security/THREAT_MODEL.md) — the
  STRIDE analysis the mandatory-auth design flows from.
