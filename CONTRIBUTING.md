# Contributing to Supernova

We welcome contributions from the community.

## How to Contribute

### Reporting Issues

- Check existing issues before creating a new one
- Provide clear description and steps to reproduce
- Include relevant system information and error messages

### Submitting Pull Requests

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes following our coding standards
4. Add tests for new functionality
5. Run tests and ensure they pass: `cargo test --workspace`
6. Run formatting: `cargo fmt --all`
7. Run clippy: `cargo clippy --all-targets`
8. Commit with clear, descriptive messages
9. Push to your fork and submit a pull request

## Development Guidelines

### Code Standards

- Follow Rust idioms and best practices.
- Write clear, self-documenting code; reserve comments for *why*, not
  *what*. The compiler tells the reader what the code does.
- Document every `pub` item — at least a one-line summary.
- Maintain test coverage on every new module. We don't enforce a
  hard percentage; we do require that every public function has at
  least one test exercising the success case and every documented
  error variant has at least one test producing it.

### Panic-free code (lint-enforced)

`supernova-core/src/lib.rs` and `node/src/lib.rs` enforce:

```rust
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), deny(clippy::expect_used))]
#![cfg_attr(not(test), deny(clippy::panic))]
#![cfg_attr(not(test), deny(clippy::unreachable))]
#![cfg_attr(not(test), deny(clippy::todo))]
#![cfg_attr(not(test), deny(clippy::unimplemented))]
```

`cfg(test)` paths are exempt — tests can use `unwrap()` for brevity.
Production paths cannot. If the build fails on one of these lints,
**don't add an `#[allow(...)]`** — fix the call site. The conversion
patterns are listed in [ADR-0009: Lock-poisoning propagation](docs/adr/0009-lock-poisoning-propagation.md).
The most common cases:

| Original | Replacement |
|---|---|
| `lock.read().unwrap()` *(in `Result`-returning fn)* | `.map_err(\|_\| ModuleError::LockPoisoned)?` (add the variant if missing) |
| `lock.read().unwrap()` *(in read-only accessor)* | `.unwrap_or_else(\|p\| p.into_inner())` |
| `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()` | `.unwrap_or(Duration::ZERO)` |
| `bincode::serialize(self).unwrap()` *(provably infallible hot path)* | `.unwrap_or_else(\|e\| { error!(...); Vec::new() })` with comment explaining why the failure arm is unreachable |
| `Some(x).unwrap()` *(after a `contains_key`-style guard)* | refactor to `if let Some(x) = ... { ... }` or `.ok_or_else(\|\| Error::...)?` |

If the right pattern isn't obvious, look at how the same lock or
fallible operation is handled in adjacent code — the workspace is
deliberately uniform so cross-referencing works.

### Submitting changes

1. Fork the repository.
2. Create a feature branch (`git checkout -b feature/amazing-feature` /
   `fix/short-bug-description` / `chore/...`).
3. Make changes following the conventions above.
4. Validation, in order:
   ```
   cargo fmt --all --check
   cargo clippy --workspace --all-features
   cargo test --workspace --release
   cargo build --workspace --release
   ```
   All four must pass. Workspace clippy is **0 deny-class errors** as
   of `v1.0.0-RC4` — keep it that way.
5. Commit messages are short imperative phrases ("propagate
   lightning poison", "build refund tx"). Architecturally significant
   decisions get an ADR (see below).
6. Push and submit a pull request.

### When to write an ADR

Architecture Decision Records live in [`docs/adr/`](docs/adr/). Write
a new one when a change:

1. Shapes a public API or wire format.
2. Changes a consensus rule or touches a cryptographic primitive.
3. Picks one irreversible option among several viable ones.
4. Establishes a workspace-wide convention (like the lock-poisoning
   policy or the bulletproof fail-closed gate).

Don't write an ADR for bug fixes, lint cleanups, doc updates, or
small refactors. The bar is "future contributors would have to
reverse-engineer this from code archaeology." Use existing ADRs as
templates; the [ADR README](docs/adr/README.md) has the format.

### Testing

- Unit tests for new functionality go in `#[cfg(test)] mod tests`
  alongside the code.
- Integration tests go in `<crate>/tests/`. Note that
  `supernova-core` has `autotests = false` — registered tests must
  be listed explicitly in `Cargo.toml`.
- Property-based tests using `proptest` are encouraged for
  invariant-driven code; existing strategies in
  `supernova-core/tests/proptests.rs`.
- Test edge cases and error variants, not just the happy path.

### Cryptographic and consensus changes

Touching anything under `supernova-core/src/{crypto, consensus,
validation}/` or anything affecting the wire format / block format
requires:

- An ADR documenting the decision.
- Test coverage for both the success case and the rejection cases
  (don't just verify "valid block accepted" — verify "invalid block
  rejected with the right error").
- Cross-validation: review with at least one second pair of eyes
  before merge.

The threat model lives at [`docs/security/THREAT_MODEL.md`](docs/security/THREAT_MODEL.md);
new attack vectors should be added there.

### Security

- Never commit sensitive information (keys, passwords, tokens).
- Report security vulnerabilities privately to security@supernovanetwork.xyz.
- Follow secure coding practices for blockchain systems.
- Operations runbooks for security incidents live in
  [`docs/operations/runbooks/`](docs/operations/runbooks/).

## Questions?

Feel free to open an issue for discussion or reach out to the maintainers.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
