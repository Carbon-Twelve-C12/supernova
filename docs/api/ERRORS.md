# Error catalogue

Every error response from the Supernova HTTP API conforms to a single
JSON shape. This page documents that shape, the stable set of error
`code` strings, and the sanitisation rules that apply to the `message`
field.

---

## Response shape

```json
{
  "status": 404,
  "message": "Block not found",
  "code": "NOT_FOUND",
  "request_id": "0e9b3f1c-8f5a-4a12-9b3f-8b3a5c91de09"
}
```

| Field | Type | Notes |
|---|---|---|
| `status` | integer | The HTTP status code, duplicated for convenience. |
| `message` | string | Human-readable explanation. Sanitised — see below. |
| `code` | string | Stable machine-readable identifier. Safe to switch on. |
| `request_id` | string \| null | Correlation ID for log lookup. Present when the request passed through the request-id middleware. |

Branch on `code`, not on the `message` text. Message wording may change
across releases; `code` values are part of the API contract and follow
the versioning rules in [`README.md`](README.md#stability-guarantees).

---

## Code catalogue

Codes are derived from HTTP status; the mapping is enforced in
`node/src/api/error.rs`.

| HTTP | `code` | When it fires |
|---|---|---|
| `400` | `BAD_REQUEST` | Malformed JSON body, missing required parameters, parameter out of bounds |
| `401` | `UNAUTHORIZED` | Missing, malformed, or unrecognised `Authorization` header |
| `403` | `FORBIDDEN` | Client IP is temporarily blocked after repeated auth failures; route requires privileges the key does not grant |
| `404` | `NOT_FOUND` | Block, transaction, channel, or other resource does not exist on this node |
| `409` | `CONFLICT` | Mutation would violate a uniqueness or state invariant (for example, opening a channel that already exists) |
| `422` | `UNPROCESSABLE_ENTITY` | Request is syntactically valid but semantically rejected (for example, a transaction that does not pass validation) |
| `429` | `RATE_LIMITED` | Global request rate limit exceeded. See [`RATE_LIMITING.md`](RATE_LIMITING.md) |
| `500` | `INTERNAL_ERROR` | Unexpected server-side fault; details are suppressed in the response and surface in node logs |
| `502` | `BAD_GATEWAY` | Upstream dependency (oracle, Lightning peer, remote watchtower) returned an error the node could not recover from |
| `503` | `SERVICE_UNAVAILABLE` | Node is syncing, storage is temporarily unreachable, or the Lightning subsystem is offline |
| `504` | `GATEWAY_TIMEOUT` | Upstream dependency exceeded the client request timeout |
| other | `UNKNOWN_ERROR` | Fallback; indicates a mapping gap worth reporting as a bug |

### Domain-specific hints

The `ApiErrorType` enum in `node/src/api/error.rs` carries richer
context for server-side logging. These do not change the `code` that
the client sees, but they guide the sanitised `message`:

| Variant | Typical `status` | Typical message fragment |
|---|---|---|
| `NodeSyncing` | `503` | `"Node is syncing"` |
| `BlockchainError` | `500`/`422` | `"Blockchain error: …"` |
| `TransactionError` | `422` | `"Transaction error: …"` |
| `MiningError` | `500` | `"Mining error: …"` |
| `NetworkError` | `503` | `"Network error: …"` |
| `EnvironmentalError` | `500` | `"Environmental error: …"` |
| `LightningError` | `503` | `"Lightning Network error: …"` |
| `WalletError` | `500`/`422` | `"Wallet error: …"` |
| `AuthorizationError` | `403` | `"Authorization error: …"` |
| `RateLimitExceeded` | `429` | `"Rate limit exceeded"` |
| `ServiceUnavailable` | `503` | `"Service unavailable: …"` |

---

## Message sanitisation

Error messages are rewritten before they leave the server. The intent
is to avoid leaking storage, credential, or infrastructure details
through error responses. The rules applied in `ApiError::sanitize_error_message`:

| Input fragment | Rewritten to |
|---|---|
| `database` | `storage` |
| `sql` | `query` |
| `password` | `credential` |
| `key` | `identifier` |
| `secret` | `credential` |
| `token` | `credential` |
| `private` | `internal` |
| `internal error` | `service error` |

Messages longer than 200 characters are truncated with an ellipsis.

Because these rewrites are string-level, they occasionally produce
awkward phrasing ("storage connection failed with credential
'…'"). That is preferable to the alternative. If a client needs the
full message for a support ticket, correlate via `request_id` and
fetch the original from the node's logs.

---

## Example responses

### 401 — missing Authorization header

```http
HTTP/1.1 401 Unauthorized
Content-Type: application/json

{
  "error": "Authentication required",
  "message": "Please provide a valid API key in the Authorization header",
  "code": "AUTH_REQUIRED"
}
```

The `401` path uses a slightly different shape (`error` rather than
`status`) because it is produced by the auth middleware before the
main error machinery runs. Both shapes carry a `code` that is stable.

### 422 — invalid transaction

```http
HTTP/1.1 422 Unprocessable Entity
Content-Type: application/json

{
  "status": 422,
  "message": "Transaction error: invalid witness program version",
  "code": "UNPROCESSABLE_ENTITY",
  "request_id": "ab12cd34-..."
}
```

### 429 — rate limit exceeded

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 60

{
  "success": false,
  "error": "Rate limit exceeded: 100 requests per 60 seconds",
  "retry_after": 60
}
```

### 503 — node syncing

```http
HTTP/1.1 503 Service Unavailable
Content-Type: application/json

{
  "status": 503,
  "message": "Node is syncing",
  "code": "SERVICE_UNAVAILABLE",
  "request_id": "fe01..."
}
```

Clients should treat `503` with the `SERVICE_UNAVAILABLE` code as a
signal to pause and re-try with exponential back-off; the sync state
typically clears on its own once block download catches up.

---

## Reporting bugs

If you hit a response with `code = "UNKNOWN_ERROR"` or a `500` whose
request pattern looks legitimate, please open an issue with:

- The `request_id` from the response (if present)
- The request method, path, and status
- The timestamp and approximate node version

Server logs are the authoritative source for the unredacted error.
Include only the `request_id` — do not share log fragments that might
contain unsanitised state.

---

## Related

- [`AUTHENTICATION.md`](AUTHENTICATION.md) — `401` and `403` behaviour.
- [`RATE_LIMITING.md`](RATE_LIMITING.md) — `429` behaviour and the
  `Retry-After` header.
- [`README.md`](README.md#stability-guarantees) — stability contract
  for the `code` field.
