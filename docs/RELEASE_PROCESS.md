# Release process

**Audience:** Supernova maintainers cutting a release, plus end users and
node operators who need to verify what they download.

The chain of trust is:

1. A **GPG-signed git tag** proves a maintainer authorised the release.
2. The release workflow **builds binaries and an SBOM** from that exact
   tag on a controlled runner.
3. **cosign keyless signatures** (Sigstore / OIDC-federated from GitHub
   Actions) bind every artifact to the workflow that produced it.
4. A **signed `SHA256SUMS` file** lets users verify all artifacts with a
   single cosign check.

If any link in that chain cannot be verified, do not trust the release.

---

## For maintainers — cutting a release

### 1. Prepare the working tree

```bash
git fetch --all
git checkout main
git pull --ff-only

cargo fmt --all --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --release
cargo audit
```

Every check must pass before proceeding. If anything fails, fix it in a
separate PR and restart.

### 2. Promote the CHANGELOG `Unreleased` section

Edit `CHANGELOG.md`:

- Add a new dated heading under the version number.
- Move every `Unreleased` entry under it.
- Leave `Unreleased` in place but empty — it reopens for the next cycle.

Commit that edit directly; do not wait for the tag.

### 3. Create a GPG-signed tag

Supernova tags are **always** GPG-signed. Unsigned tags are treated as
untrusted and will not be released.

```bash
# Ensure your release key is configured
git config user.signingkey <your-gpg-key-id>

# Sign and tag
git tag -s v1.0.0-RC5 -m "Supernova v1.0.0-RC5"

# Verify your own tag locally before pushing
git tag -v v1.0.0-RC5
```

If the verify step does not print `Good signature`, **stop**. Pushing
an unsigned or badly-signed tag pollutes the history and creates
recovery work.

### 4. Push the tag

```bash
git push origin v1.0.0-RC5
```

This triggers `.github/workflows/release.yml`. The workflow:

- Generates a CycloneDX SBOM of the full workspace.
- Builds release archives for four targets in a matrix:
  - `x86_64-unknown-linux-gnu`
  - `aarch64-unknown-linux-gnu`
  - `x86_64-apple-darwin`
  - `aarch64-apple-darwin`
- Signs each archive with cosign keyless.
- Aggregates a `SHA256SUMS` file, signs that too.
- Opens a **draft** GitHub Release containing every artifact and the
  verification snippet.

Build flags for reproducibility are pinned in the workflow
(`--remap-path-prefix` to stabilise paths; `-C strip=symbols` to drop
debug symbols). The rust toolchain version is pinned in
`.github/workflows/release.yml` (`env.RUST_VERSION`).

### 5. Review and publish the draft

Open the draft release. Verify:

- [ ] Every expected archive is present (one per target).
- [ ] Each archive has a matching `.sha256`, `.sig`, and `.pem`.
- [ ] `SHA256SUMS` / `SHA256SUMS.sig` / `SHA256SUMS.pem` are present.
- [ ] `supernova-sbom.json` is present.
- [ ] The workflow's own "Verify signatures (self-check)" step succeeded.

If anything is missing, investigate the workflow run before publishing.
Do not re-run the workflow until you have deleted the draft — otherwise
you'll double-upload.

Publish the draft when the checklist is green.

### 6. Tag-specific post-release

- Announce in the channels documented in `docs/operations/runbooks/`.
- Update `docker/README.md` if container entrypoints changed.
- For mainnet tags only (`v1.0.0` and above): the external audit report
  reference in the release notes must be filled in before publishing.

---

## For end users — verifying a downloaded release

### Prerequisites

Install `cosign` (v2+):

```bash
# macOS
brew install cosign

# Linux
# see https://docs.sigstore.dev/cosign/system_config/installation/
```

### One-shot verification of every archive

From the release page, download:

- `SHA256SUMS`
- `SHA256SUMS.sig`
- `SHA256SUMS.pem`
- The archive(s) you want for your platform.

Then:

```bash
# 1. Verify the aggregate checksum file was signed by this project's
#    release workflow on GitHub Actions.
cosign verify-blob \
  --certificate SHA256SUMS.pem \
  --signature  SHA256SUMS.sig \
  --certificate-identity-regexp 'https://github.com/Carbon-Twelve-C12/supernova/\.github/workflows/release\.yml@.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  SHA256SUMS

# 2. Verify that each archive you downloaded matches the signed sums.
sha256sum -c --ignore-missing SHA256SUMS
```

Both steps must print `OK` / `Verified OK`. If either fails, **do not
install the binary.**

The `certificate-identity-regexp` ties the signature to this repository's
release workflow; anyone who obtains an OIDC token from elsewhere cannot
produce a signature that satisfies it.

### Optional: verify a single archive directly

```bash
cosign verify-blob \
  --certificate supernova-<version>-<target>.tar.gz.pem \
  --signature  supernova-<version>-<target>.tar.gz.sig \
  --certificate-identity-regexp 'https://github.com/Carbon-Twelve-C12/supernova/\.github/workflows/release\.yml@.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  supernova-<version>-<target>.tar.gz
```

### Verifying the git tag

```bash
git fetch --tags
git tag -v v1.0.0-RC5
```

The output should end with `Good signature from "<maintainer>"` and the
key fingerprint should match the one published in the project's
communications. If the key is not yet in your keyring, fetch it from a
public keyserver and cross-check the fingerprint via an out-of-band
channel before trusting the tag.

### Verifying the Docker image

Tagged images on Docker Hub are also cosign-signed. To verify:

```bash
cosign verify \
  --certificate-identity-regexp 'https://github.com/Carbon-Twelve-C12/supernova/\.github/workflows/docker-image\.yml@.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  docker.io/carbon-twelve-c12/supernova:<tag>
```

Image signatures bind to the image's content-addressable digest, so a
tag substitution attack between `docker pull` and verification is
detectable.

---

## Distribution channels

| Channel | Purpose | Signed? |
|---|---|---|
| **GitHub Releases** | Canonical source for binaries, SBOM, signatures | Yes (cosign) |
| **Docker Hub** (`carbon-twelve-c12/supernova`) | Container images | Yes (cosign) |
| Homebrew tap | Convenience for macOS operators | Planned |
| Linux packages (`.deb`, `.rpm`) | Convenience for server operators | Planned |

Everything outside the Signed column requires an out-of-band trust
decision; operators running mainnet infrastructure should stick to
channels whose signatures can be verified.

---

## Recovery from a bad release

If a published release turns out to be broken or compromised:

1. **Delete the draft** if it has not been published.
2. **Do not delete a published release.** Instead, publish an advisory
   in `docs/security/` and cut a new release with the fix. Deleting
   published artifacts breaks verification for every user who already
   downloaded them.
3. **Rotate the cosign OIDC context is not necessary** — Sigstore logs
   every signature in a public transparency log. If an unauthorised
   signature somehow existed, the transparency log would be the
   authoritative record for audit.
4. **GPG key compromise** requires a coordinated key-rotation and
   re-sign of unaffected recent tags. See the incident-response runbook
   in `docs/operations/runbooks/`.

---

## References

- Cosign / Sigstore: https://docs.sigstore.dev/
- CycloneDX SBOM: https://cyclonedx.org/specification/overview/
- `.github/workflows/release.yml` — the canonical workflow this document
  describes.
- `.github/workflows/docker-image.yml` — image-signing pipeline.
- [`CHANGELOG.md`](../CHANGELOG.md) — release checklist is also mirrored
  there for convenience.
