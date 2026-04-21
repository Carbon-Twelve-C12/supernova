# Supernova Node Operator Guide

**Audience:** Anyone standing up a Supernova node, from a hobbyist
testnet validator to a professional operator running mainnet
infrastructure.

This is the canonical operator document. Other docs in the tree go
deeper on specific topics — this guide is the spine that ties them
together and explains **when** each one matters.

Related documents (read in this order as needed):

- [`RELEASE_PROCESS.md`](RELEASE_PROCESS.md) — how releases are
  produced and verified.
- [`operations/PERFORMANCE_TUNING.md`](operations/PERFORMANCE_TUNING.md) —
  hardware, kernel, and configuration tuning.
- [`operations/BACKUP_RESTORE.md`](operations/BACKUP_RESTORE.md) —
  backup cadence, restore procedure.
- [`operations/DISASTER_RECOVERY.md`](operations/DISASTER_RECOVERY.md) —
  when everything goes wrong.
- [`operations/runbooks/`](operations/runbooks/) — one runbook per
  P0/P1 alert class.
- [`security/THREAT_MODEL.md`](security/THREAT_MODEL.md) — what the
  protocol defends against.

---

## Contents

1. [Quick start](#quick-start)
2. [Before you start](#before-you-start)
3. [Installing the binary](#installing-the-binary)
4. [First boot](#first-boot)
5. [Joining the network](#joining-the-network)
6. [Monitoring](#monitoring)
7. [Day-two operations](#day-two-operations)
8. [Upgrading](#upgrading)
9. [Troubleshooting decision tree](#troubleshooting-decision-tree)

---

## Quick start

For an Ubuntu 22.04 LTS server with the binary on your `$PATH` and a
valid `/etc/supernova/node.toml`:

```bash
sudo systemctl enable --now supernova-node
sudo systemctl status  supernova-node
supernova-cli getblockchaininfo
```

If all three succeed and the third shows an increasing `blocks` height,
you're synced. If not, work through the sections below.

---

## Before you start

### Hardware sizing

Supernova's post-quantum signatures cost more CPU to verify than classical
secp256k1 — plan accordingly.

| Role | CPU | RAM | Storage | Network |
|---|---|---|---|---|
| **Testnet node** | 4 cores | 8 GB | 100 GB SSD | 100 Mbps |
| **Mainnet full node** | 8 cores | 16 GB | 500 GB NVMe | 1 Gbps |
| **Miner** | 16+ cores | 32 GB | 500 GB NVMe | 1 Gbps, low-latency |
| **Archive / analytics** | 8 cores | 32 GB | 2 TB NVMe | 1 Gbps |

Detailed tuning (THP, file descriptors, I/O scheduler, network buffers):
see [`PERFORMANCE_TUNING.md`](operations/PERFORMANCE_TUNING.md).

### Operating system

- **Officially supported:** Ubuntu 22.04 LTS (amd64 and arm64).
- **Known-good:** Debian 12, RHEL 9 derivatives.
- **Unsupported:** Windows (not tested on the release path; Linux under
  WSL may work but is not a supported configuration for production).

### Network

| Port | Purpose | Exposure |
|---|---|---|
| `8000/tcp` | P2P (libp2p TCP) | **Public** — incoming peers |
| `9000/tcp` | Prometheus metrics | **Private** — localhost or VPN only |
| `8332/tcp` | JSON-RPC / REST API | **Private by default** — only expose behind a reverse proxy with auth |

At minimum, open `8000/tcp` inbound. Never expose the metrics or API
ports directly to the internet — the API is authenticated but the
metrics endpoint is not.

### Service account

Run under a dedicated non-root user. Never run the node as `root`.

```bash
sudo useradd -r -m -s /bin/bash supernova
sudo mkdir -p /data/supernova /etc/supernova /var/log/supernova
sudo chown -R supernova:supernova /data/supernova /var/log/supernova
sudo chmod 750 /data/supernova /etc/supernova
```

---

## Installing the binary

**Always verify signatures.** The release workflow publishes cosign
signatures for every artifact and a signed `SHA256SUMS`. Skipping
verification means trusting your mirror.

### Option A — GitHub Releases (recommended)

```bash
VERSION="v1.0.0-RC5"   # adjust to the release you want
TARGET="x86_64-unknown-linux-gnu"
BASE="https://github.com/Carbon-Twelve-C12/supernova/releases/download/${VERSION}"

# Fetch archive, signature, and signed checksums
curl -L -o archive.tar.gz      "${BASE}/supernova-${VERSION}-${TARGET}.tar.gz"
curl -L -o archive.tar.gz.sig  "${BASE}/supernova-${VERSION}-${TARGET}.tar.gz.sig"
curl -L -o archive.tar.gz.pem  "${BASE}/supernova-${VERSION}-${TARGET}.tar.gz.pem"
curl -L -o SHA256SUMS          "${BASE}/SHA256SUMS"
curl -L -o SHA256SUMS.sig      "${BASE}/SHA256SUMS.sig"
curl -L -o SHA256SUMS.pem      "${BASE}/SHA256SUMS.pem"
```

Verify with cosign (see [`RELEASE_PROCESS.md`](RELEASE_PROCESS.md) for
the full command and flags):

```bash
cosign verify-blob \
  --certificate SHA256SUMS.pem \
  --signature  SHA256SUMS.sig \
  --certificate-identity-regexp 'https://github.com/Carbon-Twelve-C12/supernova/\.github/workflows/release\.yml@.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  SHA256SUMS

sha256sum -c --ignore-missing SHA256SUMS
```

Both checks must pass. Then:

```bash
tar -xzf archive.tar.gz
sudo install -m 0755 -o root -g root \
  supernova-*/supernova-node \
  supernova-*/supernova-cli \
  /usr/local/bin/
```

### Option B — Docker

Signed multi-arch images are published to Docker Hub:

```bash
cosign verify \
  --certificate-identity-regexp 'https://github.com/Carbon-Twelve-C12/supernova/\.github/workflows/docker-image\.yml@.*' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  docker.io/carbon-twelve-c12/supernova:v1.0.0-RC5

docker pull docker.io/carbon-twelve-c12/supernova:v1.0.0-RC5
```

See `docker/docker-compose.yml` for a ready-to-run node + monitoring
stack.

### Option C — Build from source

For auditors and contributors only. Reproducibility is **not** asserted
for source builds — only the release-workflow artifacts carry that
claim.

```bash
git clone https://github.com/Carbon-Twelve-C12/supernova
cd supernova
git verify-tag v1.0.0-RC5     # requires the project's GPG key in your keyring
cargo build --release --all-features
```

---

## First boot

### Configuration

Copy the example config and edit for your environment:

```bash
sudo cp config/node.example.toml     /etc/supernova/node.toml   # mainnet or default
sudo cp config/testnet.example.toml  /etc/supernova/node.toml   # testnet
sudo chown supernova:supernova       /etc/supernova/node.toml
sudo chmod 640                        /etc/supernova/node.toml
```

Required edits before first boot:

| Key | What to set | Notes |
|---|---|---|
| `node.chain_id` | `"supernova-mainnet"` or `"supernova-testnet"` | Must match the network you intend to join |
| `node.environment` | `"Production"` / `"Testnet"` / `"Development"` | Controls safety defaults |
| `network.listen_addr` | `/ip4/0.0.0.0/tcp/8000` | Bind on the public interface if you want inbound peers |
| `network.bootstrap_nodes` | List of `/ip4/.../tcp/8000/p2p/<PeerId>` | Seed list for initial discovery; see below |
| `storage.db_path` | `/data/supernova/db` | Must be owned by the `supernova` user |
| `backup.backup_dir` | `/data/supernova/backups` | Same ownership constraint |
| `mining.enable` | `false` unless you intend to mine | Mining should usually run as a separate `miner` process, not in the node |

The full field reference is in `config/node.example.toml`; every option
has an inline comment.

### Bootstrap nodes

Seed peers by network:

- **Mainnet:** published at https://supernovanetwork.xyz and also baked
  into default builds when `node.environment = "Production"`.
- **Testnet:** `seed.testnet.supernovanetwork.xyz:8000`
  (DNS-A-record multiaddress). The testnet config example uses this by
  default.

For an isolated dev network, leave `bootstrap_nodes` empty and connect
nodes to each other with explicit multiaddrs.

### systemd service

The recommended service unit:

```ini
# /etc/systemd/system/supernova-node.service
[Unit]
Description=Supernova full node
After=network-online.target
Wants=network-online.target

[Service]
Type=exec
User=supernova
Group=supernova
ExecStart=/usr/local/bin/supernova-node --config /etc/supernova/node.toml
Restart=on-failure
RestartSec=10
LimitNOFILE=65536
LimitNPROC=16384

# Hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/data/supernova /var/log/supernova
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
RestrictRealtime=true
RestrictNamespaces=true
LockPersonality=true

# Resource limits (adjust to host)
MemoryMax=16G
TasksMax=8192

[Install]
WantedBy=multi-user.target
```

Then:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now supernova-node
sudo systemctl status supernova-node
journalctl -u supernova-node -f
```

### Node identity

On first boot, the node generates a libp2p keypair under
`network.key_path` (default: `/data/supernova/keys/node.key`). This key
identifies your node's PeerId on the network.

- **Back it up.** Losing it does not lose funds but changes your PeerId,
  so peer-reputation and static-peer configurations on your peers will
  need to be refreshed.
- **Do not share it.** The private key is your peer identity; disclosing
  it lets someone impersonate your node.

---

## Joining the network

Check sync progress after the service starts:

```bash
supernova-cli getblockchaininfo
```

Expected output (abridged):

```json
{
  "chain":          "supernova-testnet",
  "blocks":         12345,
  "headers":        12345,
  "bestblockhash":  "...",
  "verificationprogress": 1.0,
  "initial_block_download": false
}
```

Healthy-sync indicators:

- `blocks` increases roughly at the configured block-time cadence
  (150 s).
- `peers` count (`supernova-cli getpeerinfo | jq 'length'`) is ≥ 8.
- `verificationprogress` approaches 1.0 and `initial_block_download`
  transitions to `false`.

If sync stalls, see the [Troubleshooting decision tree](#troubleshooting-decision-tree).

---

## Monitoring

Supernova exposes Prometheus metrics on `127.0.0.1:9000/metrics` by
default. A reference dashboard and alert rules are in the repo:

- `deployment/monitoring/prometheus/prometheus.yml`
- `deployment/monitoring/prometheus/alerts.yml`
- `docker/docker-compose.yml` (runs Prometheus + Grafana + AlertManager
  alongside the node)

At minimum, scrape and alert on:

| Metric / alert | What it tells you |
|---|---|
| `supernova_blocks_height` | Is the node still advancing? |
| `supernova_peers_total`   | Is the node well-connected? |
| `supernova_mempool_size`  | Is the mempool at / near capacity? |
| `ConsensusHalt`           | No new blocks for > 30 min |
| `ChainTipDivergence`      | Local tip disagrees with reference set |
| `UTXOSetCorruption`       | Storage checksum mismatch |
| `QuantumCanaryTriggered`  | Quantum-canary subsystem fired |
| `ForkDetected`            | Competing chain tip observed |
| `BackupCriticallyOld`     | No successful backup in > 24h |

Each alert routes to a specific runbook — see
[`operations/runbooks/README.md`](operations/runbooks/README.md).

For privacy, metrics go through a redaction filter
(`node/src/metrics/privacy.rs`) that strips peer IPs and
transaction-level identifiers. If you ship metrics to a third party,
review what the scrape actually contains before enabling.

---

## Day-two operations

### Backups

Automated backups are on by default (`backup.enable_automated_backups =
true`). The full procedure — cadence, retention, verification, restore
drill — is in [`operations/BACKUP_RESTORE.md`](operations/BACKUP_RESTORE.md).

Minimum discipline:

1. Back up to a volume on a different physical host.
2. Run a **restore rehearsal** quarterly. A backup you have never
   restored is a hope, not a backup.
3. Watch the `BackupCriticallyOld` alert; do not silence it.

### Pruning

For non-archive nodes, pruning reclaims disk by dropping fully-validated
blocks once the UTXO set commits are final. Configure in the
`[storage]` section (see `config/node.example.toml`).

### Log management

Logs go to stderr by default; `journalctl` handles rotation under
systemd. For high-volume nodes, ship logs to a central aggregator
(rsyslog → Loki / Elasticsearch). Node logs do **not** contain keys or
seed phrases — the redaction filter `logging::redaction` strips them
before they reach stdout.

---

## Upgrading

Supernova releases follow semver with `-RCN` suffixes. Minor and RC
bumps are **expected to be drop-in**: stop, replace binary, start. Major
bumps may require migration — always read the CHANGELOG first.

Procedure:

1. Read [`CHANGELOG.md`](../CHANGELOG.md) for the target version. Note
   any migration steps.
2. Back up (`supernova-cli backup create --out /data/supernova/backups/pre-upgrade.tar`).
3. Fetch and verify the new binary (see
   [Installing the binary](#installing-the-binary)).
4. Stop the node: `sudo systemctl stop supernova-node`.
5. Swap in the new binary: `sudo install -m 0755 supernova-node /usr/local/bin/supernova-node`.
6. Start: `sudo systemctl start supernova-node`.
7. Confirm: `supernova-cli getblockchaininfo` shows increasing height
   and `peers` ≥ 8 within two minutes.

If step 7 fails within 10 minutes, roll back to the previous binary and
open an issue with logs before retrying.

---

## Troubleshooting decision tree

Work top-to-bottom. The first matching symptom wins.

### 1. Node won't start

```
journalctl -u supernova-node --since "5 minutes ago"
```

| Log signal | Likely cause | Action |
|---|---|---|
| `address already in use` | Port 8000 held by another process | `sudo ss -lptn | grep :8000`; stop the conflicting process or change `network.listen_addr`. |
| `Permission denied` on `db_path` | Data dir wrong owner/mode | `sudo chown -R supernova:supernova /data/supernova` |
| `config validation failed: ...` | TOML schema error | Open `/etc/supernova/node.toml`, match the field to `config/node.example.toml` |
| `storage corruption` on boot | UTXO checksum mismatch | Run the [DATABASE_CORRUPTION runbook](operations/runbooks/DATABASE_CORRUPTION.md) |

### 2. Node starts but does not sync

Check `supernova-cli getpeerinfo | jq 'length'`:

- **Zero peers** → bootstrap problem. Verify `bootstrap_nodes` entries
  resolve, firewall allows outbound `8000/tcp`, and
  `network.network_id` matches the target chain.
- **Peers present, `blocks` not advancing** → headers vs blocks
  divergence. Check `supernova-cli getchaintips` for a competing tip.
  If competing, see [FORK_RESOLUTION runbook](operations/runbooks/FORK_RESOLUTION.md).
- **Peers present, `initial_block_download = true` for hours** → normal
  for first sync on modest hardware. Grafana "sync rate"
  panel tells you whether it's actually stuck or merely slow.

### 3. Node was synced, now falling behind

| Signal | Runbook |
|---|---|
| `ConsensusHalt` alert fired | [CONSENSUS_HALT](operations/runbooks/CONSENSUS_HALT.md) |
| Chain tip different from public explorer | [NETWORK_PARTITION](operations/runbooks/NETWORK_PARTITION.md) |
| `ForkDetected` alert fired | [FORK_RESOLUTION](operations/runbooks/FORK_RESOLUTION.md) |
| Storage errors in logs | [DATABASE_CORRUPTION](operations/runbooks/DATABASE_CORRUPTION.md) |
| Peer count collapsed | Check for outbound firewall / ISP change; see eclipse-prevention section of the threat model |

### 4. Peers dropping or banning us

Most commonly one of:

- Time skew → `sudo timedatectl status`; fix NTP if drifting.
- Rate-limit tripped by a local integration flooding our node → check
  the API rate-limiter logs and back the client off.
- Our PeerId got banned on the network → rotate node identity (delete
  `network.key_path` and restart; note this changes your PeerId).

### 5. QuantumCanaryTriggered

This is a **P0** alert indicating the quantum-canary subsystem detected
a signature-verification anomaly suggesting a potential crypto break
or implementation bug. Follow the
[QUANTUM_EMERGENCY runbook](operations/runbooks/QUANTUM_EMERGENCY.md)
immediately; do not wait for further signal.

### 6. API returning 5xx

```bash
supernova-cli getblockchaininfo   # does the direct CLI work?
```

- CLI works, API fails → the API server (`node/src/api/`) is unhealthy
  but the core node isn't. Restart the node; if it recurs, capture
  logs and open an issue.
- CLI also fails → the node itself is unhealthy. Go back to section 1.

### 7. Nothing fits

Capture state and open an issue:

```bash
journalctl -u supernova-node --since "1 hour ago" > /tmp/node.log
supernova-cli getblockchaininfo > /tmp/blockchain.json
supernova-cli getpeerinfo        > /tmp/peers.json
supernova-cli getmempoolinfo     > /tmp/mempool.json
```

Attach those four files to a GitHub issue with a short description of
what changed before the problem started. Do **not** attach
`node.toml` — it may contain sensitive endpoints or credentials.

For suspected security issues, follow responsible-disclosure: do not
open a public issue. See `SECURITY.md` for the reporting path.

---

## Appendices

### A. Common CLI commands

```bash
supernova-cli getblockchaininfo       # chain height, tip, sync state
supernova-cli getpeerinfo             # connected peers
supernova-cli getmempoolinfo          # mempool size, fee stats
supernova-cli getmininginfo           # mining state (if enabled)
supernova-cli getchaintips            # all known chain tips (fork detection)
supernova-cli repair check            # quick integrity check
supernova-cli repair check --level full
supernova-cli backup create --out <path>
supernova-cli backup verify <path>
```

### B. Useful environment variables

| Variable | Purpose |
|---|---|
| `RUST_LOG=info,supernova=debug` | Fine-grained log levels |
| `SUPERNOVA_CONFIG=/path/to/node.toml` | Override default config path |
| `SUPERNOVA_NETWORK=testnet` | Override network (also set in config) |

### C. On-call checklist

Before taking a Supernova on-call shift:

- [ ] Paged on `supernova-alerts` channel / PagerDuty service.
- [ ] Read every runbook in `operations/runbooks/` at least once.
- [ ] Confirmed credentials / access for: Grafana, log aggregator, seed
  node SSH, Docker Hub registry.
- [ ] Ran `supernova-cli` against a reference mainnet or testnet node
  from your laptop successfully.
- [ ] Know the escalation path (see each runbook's Escalation section).

A checklist you've never worked through is not preparation. Run a
tabletop against one runbook per week during shift rotation.
