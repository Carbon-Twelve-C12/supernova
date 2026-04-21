# Rate limiting

The Supernova HTTP API enforces a per-client-IP sliding-window rate
limit on every authenticated route. This page documents the window
semantics, the response headers clients should react to, and which
routes bypass the limit.

A separate rate limiter covers authentication attempts; see
[`AUTHENTICATION.md`](AUTHENTICATION.md) §4 for that policy.

---

## 1. Policy

| Parameter | Default | Configured by |
|---|---|---|
| Rate | 100 requests | `api.rate_limit` in node config |
| Window | 60 seconds | Fixed |
| Scope | Per client IP | `peer_addr` as reported by `actix_web::connection_info()` |
| Clean-up | Lazy; expired entries dropped on next access | Not configurable |

The window is rolling per-client — not per-minute wallclock. Your first
request starts a 60-second window, and every subsequent request from
the same IP falls into the same window until it expires.

### Configuration example

```toml
[api]
rate_limit = 500    # 500 req/min/ip, up from the default 100
```

Set higher limits only on nodes that front an explorer or indexer; a
conservative default for production nodes serving the general internet
is `60–120`.

---

## 2. Response headers

Every response — `2xx`, `4xx`, or `5xx` — carries these headers when
the route is subject to the limiter:

| Header | Meaning |
|---|---|
| `X-RateLimit-Limit` | Requests allowed in the current window |
| `X-RateLimit-Remaining` | Requests remaining in the current window |
| `X-RateLimit-Reset` | Unix timestamp (seconds) when the window resets |

On a `429 Too Many Requests`, the node additionally emits:

| Header | Meaning |
|---|---|
| `Retry-After` | Seconds until the window resets. Clients should back off for at least this long. |

### Example

```
HTTP/1.1 200 OK
Content-Type: application/json
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 97
X-RateLimit-Reset: 1750000860
```

### Example 429 body

```json
{
  "success": false,
  "error": "Rate limit exceeded: 100 requests per 60 seconds",
  "retry_after": 60
}
```

---

## 3. Bypass list

Routes that must remain reachable regardless of client volume are
exempted from the global limiter:

| Route prefix | Why |
|---|---|
| `/swagger-ui/` | Documentation must stay reachable during incident investigation. |
| `/api-docs/` | Same reason; OpenAPI JSON is a static artifact. |

Everything else goes through the limiter. `OPTIONS` pre-flight requests
are also bypassed so that CORS negotiation does not consume budget.

**`GET /health` is not on this list** — liveness probes are usually
internal and should not saturate a node's rate budget. If you put a
public load balancer probe on `/health`, configure the probe's shared
IP in the rate-limit exceptions at the proxy layer, not at the node.

---

## 4. Client behaviour

Well-behaved clients should:

1. Read `X-RateLimit-Remaining` on every response and pre-emptively
   slow down when it approaches zero.
2. On `429`, stop issuing requests until at least `Retry-After` seconds
   have elapsed. Retrying sooner only restarts the rejection stream.
3. Apply jitter when multiple clients share an egress IP — otherwise
   every client retries at the same reset time and immediately triggers
   another round of `429`s.
4. Treat any `5xx` + `Retry-After` response the same way: back off for
   the advertised duration.

### Reference back-off

```python
import time, requests, random

def call(url, headers):
    while True:
        r = requests.get(url, headers=headers)
        if r.status_code != 429:
            return r
        wait = int(r.headers.get("Retry-After", "60"))
        time.sleep(wait + random.uniform(0, 2))
```

---

## 5. Operator notes

- The limiter keeps its state in process memory. Restarting the node
  clears every client's counter — expected behaviour, not a bug.
- Horizontally scaled deployments should run a request-rate-aware
  reverse proxy (for example, nginx with `limit_req_zone`, or an
  upstream WAF) so that limits are enforced across node replicas
  rather than per-replica.
- Lock poisoning on the rate-limit state (for example, after a panic
  in a handler) degrades the service gracefully: the limiter logs the
  event and lets requests through rather than hard-failing the API.
  Watch for `rate_limiter state lock poisoned` in the logs — it
  usually indicates a separate upstream bug worth investigating.

---

## Related

- [`AUTHENTICATION.md`](AUTHENTICATION.md) — the authentication-attempt
  limiter that protects the key-check path.
- [`ERRORS.md`](ERRORS.md) — `429` response shape and error code.
- [`../NODE_OPERATOR_GUIDE.md`](../NODE_OPERATOR_GUIDE.md) — where
  `api.rate_limit` lives in the shipped config.
