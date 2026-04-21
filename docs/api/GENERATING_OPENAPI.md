# Generating the OpenAPI specification

The canonical OpenAPI 3.0 specification for the Supernova HTTP API is
derived from the `utoipa` annotations on the handler and type
definitions in the `node` crate. It is not hand-maintained.

This page explains:

1. Where the spec lives at runtime
2. How to export it to a file for offline review, diffing, or SDK
   generation
3. The guarantee maintainers must uphold when changing the API

---

## 1. Runtime endpoints

A running node publishes the spec and a Swagger UI viewer:

| Path | Serves |
|---|---|
| `/api-docs/openapi.json` | Machine-readable OpenAPI 3.0 JSON |
| `/swagger-ui/` | Interactive Swagger UI against the spec above |

Both paths bypass the global rate limiter so that documentation remains
reachable during incident investigation. They still respect the
authentication policy if auth is enabled — treat them as internal tools
unless you have explicitly opened them up at the reverse proxy.

### Fetching the spec from a running node

```bash
curl -s \
  -H "Authorization: Bearer $SUPERNOVA_API_KEY" \
  http://127.0.0.1:8080/api-docs/openapi.json \
  > openapi.json
```

---

## 2. Exporting from source

For SDK generation and spec-diff review in CI, the spec can be emitted
without running a full node. The `node` crate ships an example binary
that prints the same document the runtime endpoint returns.

### Generate locally

```bash
# From the repository root
cargo run -p node --example emit_openapi --release > docs/api/openapi.json
```

The output is pretty-printed JSON and deterministic for a given commit,
so checking it in produces clean diffs when the API changes.

### Validate the output

```bash
# Requires: npm i -g @apidevtools/swagger-cli
swagger-cli validate docs/api/openapi.json
```

Or, to regenerate and compare in one step (exits non-zero on drift):

```bash
cargo run -p node --example emit_openapi --release \
  | diff -u docs/api/openapi.json -
```

This form is appropriate as a CI guard: it fails the build when
handler annotations change without a corresponding spec refresh.

---

## 3. Generating client SDKs

The `openapi.json` file is a standard OpenAPI 3.0 document and can be
fed into any generator. Some common targets:

```bash
# TypeScript (openapi-typescript-codegen)
npx openapi-typescript-codegen \
  --input docs/api/openapi.json \
  --output sdk/ts

# Python (openapi-generator-cli)
openapi-generator-cli generate \
  -i docs/api/openapi.json \
  -g python \
  -o sdk/python

# Go (oapi-codegen)
oapi-codegen -package supernovaapi \
  docs/api/openapi.json > sdk/go/client.go
```

Generated SDKs live outside the main repository by convention — the
spec is the contract, individual SDK flavours are not.

---

## 4. Maintainer guarantee

When changing any `#[utoipa::path]` or response type:

1. Regenerate the spec (`cargo run -p node --example emit_openapi ...`).
2. Review the diff. Ensure no field was silently removed or retyped.
3. Commit the regenerated `docs/api/openapi.json` in the same commit
   as the code change, so reviewers see the contract change alongside
   the implementation.
4. If the change is breaking — field removal, type change, required
   parameter addition, authentication tightening — bump the URL
   version prefix and follow the deprecation policy in
   [`README.md`](README.md#stability-guarantees).

The CI pipeline gates breaking changes by running the `diff` form in
step 2 above; new versions of the spec are shipped as release
artifacts alongside binaries.

---

## Related

- [`README.md`](README.md) — API overview and stability guarantees.
- `node/src/api/docs/openapi.rs` — the `utoipa::OpenApi` derive that
  drives generation.
- `node/examples/emit_openapi.rs` — the exporter referenced above.
