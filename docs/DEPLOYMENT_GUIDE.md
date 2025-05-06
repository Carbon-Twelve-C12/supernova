# Supernova Blockchain Deployment Guide

This comprehensive guide covers various deployment options for the Supernova blockchain, from development setups to production-grade deployments.

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Docker Deployment](#docker-deployment)
3. [Kubernetes Deployment](#kubernetes-deployment)
4. [Bare Metal Deployment](#bare-metal-deployment)
5. [Monitoring and Alerting](#monitoring-and-alerting)
6. [Backup and Recovery](#backup-and-recovery)
7. [Security Best Practices](#security-best-practices)
8. [Troubleshooting](#troubleshooting)
9. [Upgrade Procedures](#upgrade-procedures)

## System Requirements

### Minimum Requirements (Development/Testing)

- **CPU**: 4 cores
- **RAM**: 8 GB
- **Storage**: 100 GB SSD
- **Network**: 10 Mbps, stable connection
- **Operating System**: Ubuntu 20.04+, Debian 11+, or CentOS 8+

### Recommended Requirements (Production)

- **CPU**: 16+ cores
- **RAM**: 32+ GB
- **Storage**: 1+ TB NVMe SSD
- **Network**: 100+ Mbps, low-latency, stable connection
- **Operating System**: Ubuntu 22.04 LTS

### Mining Node Requirements

- **CPU**: 16+ cores optimized for mining
- **RAM**: 16+ GB
- **Storage**: 500+ GB NVMe SSD
- **Network**: 100+ Mbps, low-latency, stable connection
- **Operating System**: Ubuntu 22.04 LTS

## Docker Deployment

### Prerequisites

- Docker 20.10.x or newer
- Docker Compose 2.10.x or newer
- Git
- Basic understanding of Docker concepts

### Single Node Setup

1. **Clone the Repository**

```bash
git clone https://github.com/username/supernova.git
cd supernova
```

2. **Build the Docker Image**

```bash
docker build -t supernova:latest -f docker/Dockerfile .
```

3. **Configure the Node**

```bash
# Copy example configuration
cp config/node.example.toml config/node.toml

# Edit configuration as needed
nano config/node.toml
```

4. **Run a Single Node**

```bash
docker run -d --name supernova-node \
  -p 9333:9333 -p 9332:9332 -p 9090:9090 \
  -v $(pwd)/data:/home/supernova/data \
  -v $(pwd)/config/node.toml:/home/supernova/config/default.toml \
  supernova:latest
```

5. **Check Node Status**

```bash
docker logs -f supernova-node
```

### Multi-Node Setup with Docker Compose

1. **Configure Docker Compose**

```bash
# Create directories for data and logs
mkdir -p data/{node1,node2,miner} logs/{node1,node2,miner}

# Configure network settings for each node in docker-compose.yml
```

2. **Start the Multi-Node Network**

```bash
docker-compose -f docker/docker-compose.yml up -d
```

3. **Monitor Logs**

```bash
docker-compose -f docker/docker-compose.yml logs -f
```

4. **Stop the Network**

```bash
docker-compose -f docker/docker-compose.yml down
```

### Environment Variables

Key environment variables that can be used to configure the Docker containers:

| Variable | Description | Default Value |
|----------|-------------|---------------|
| `RUST_LOG` | Log level (trace, debug, info, warn, error) | info |
| `NODE_NAME` | Name of the node | supernova-node |
| `NETWORK` | Network to connect to (mainnet, testnet) | mainnet |
| `MINE` | Enable mining | false |
| `RUST_BACKTRACE` | Enable backtraces on errors | 0 |
| `SUPERNOVA_CONFIG_DIR` | Configuration directory | /home/supernova/config |
| `SUPERNOVA_DATA_DIR` | Data directory | /home/supernova/data |
| `SUPERNOVA_CHECKPOINTS_DIR` | Checkpoint directory | /home/supernova/checkpoints |
| `SUPERNOVA_BACKUPS_DIR` | Backup directory | /home/supernova/backups |

## Kubernetes Deployment

### Prerequisites

- Kubernetes cluster 1.20+
- Helm 3.1.0+
- kubectl configured to communicate with your cluster
- Persistent volume provisioner support
- LoadBalancer support (optional)

### Installation with Helm

1. **Add the Supernova Helm Repository**

```bash
helm repo add supernova https://charts.supernovanetwork.xyz
helm repo update
```

2. **Create a Values File**

Create a `values.yaml` file to customize your deployment:

```yaml
# Example values.yaml
global:
  environment: production
  storageClass: "standard"  # Use your storage class

fullNode:
  replicaCount: 3
  autoscaling:
    enabled: true
    minReplicas: 3
    maxReplicas: 10
  resources:
    requests:
      cpu: "2"
      memory: "4Gi"
    limits:
      cpu: "4"
      memory: "8Gi"

miner:
  enabled: true
  replicaCount: 2
  resources:
    requests:
      cpu: "4"
      memory: "8Gi"
    limits:
      cpu: "8"
      memory: "16Gi"

prometheus:
  enabled: true

alertManager:
  enabled: true
  receivers:
    email:
      enabled: true
      to: "alerts@yourdomain.com"
    slack:
      enabled: true
      webhookUrl: "https://hooks.slack.com/services/XXXX/YYYY/ZZZZ"
      channel: "#alerts"

grafana:
  enabled: true
  adminPassword: "YourSecurePassword"  # Change this
  ingress:
    enabled: true
    hosts:
      - host: metrics.yourdomain.com
        paths:
          - path: /
            pathType: Prefix
```

3. **Install the Helm Chart**

```bash
helm install supernova supernova/supernova -f values.yaml
```

4. **Check Deployment Status**

```bash
kubectl get pods -l app.kubernetes.io/instance=supernova
```

5. **Access Services**

```bash
# Get the URL for Grafana dashboard
kubectl get ingress -l app.kubernetes.io/instance=supernova,app.kubernetes.io/component=grafana

# Get RPC service details
kubectl get svc -l app.kubernetes.io/instance=supernova,app.kubernetes.io/component=full-node
```

### Manual Kubernetes Deployment

If you prefer to deploy without Helm, you can use the provided Kubernetes manifests:

1. **Apply the Namespace**

```bash
kubectl apply -f kubernetes/supernova-deployment.yaml
```

2. **View Deployment Status**

```bash
kubectl get all -n supernova
```

### Kubernetes Resource Management

Proper resource management is crucial for a stable blockchain node:

1. **Resource Requests and Limits**
   - Set appropriate CPU and memory requests and limits for each component
   - Monitor actual usage and adjust as needed

2. **Persistent Volume Claims**
   - Use high-performance storage for blockchain data
   - Ensure sufficient capacity for growth

3. **Auto-Scaling**
   - Configure horizontal pod autoscaling based on CPU/memory utilization
   - Set appropriate minimum and maximum replica counts

## Bare Metal Deployment

### Prerequisites

- Ubuntu 22.04 LTS or similar server OS
- User with sudo privileges
- Firewall access for required ports

### Installation Steps

1. **Install Dependencies**

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev git curl
```

2. **Install Rust**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

3. **Clone the Repository**

```bash
git clone https://github.com/username/supernova.git
cd supernova
```

4. **Build the Project**

```bash
cargo build --release
```

5. **Create Configuration**

```bash
mkdir -p ~/.supernova/config
cp config/node.example.toml ~/.supernova/config/node.toml
# Edit the configuration file
nano ~/.supernova/config/node.toml
```

6. **Create Systemd Service**

Create a file at `/etc/systemd/system/supernova.service`:

```ini
[Unit]
Description=Supernova Blockchain Node
After=network.target

[Service]
Type=simple
User=YOUR_USERNAME
ExecStart=/path/to/supernova/target/release/node --config /home/YOUR_USERNAME/.supernova/config/node.toml
Restart=on-failure
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

7. **Enable and Start the Service**

```bash
sudo systemctl daemon-reload
sudo systemctl enable supernova
sudo systemctl start supernova
```

8. **Check Service Status**

```bash
sudo systemctl status supernova
journalctl -u supernova -f
```

### Firewall Configuration

Open the necessary ports for Supernova:

```bash
sudo ufw allow 9333/tcp  # P2P networking
sudo ufw allow 9332/tcp  # RPC (restrict to trusted IPs in production)
```

## Monitoring and Alerting

### Prometheus and Grafana Setup

1. **Configure Prometheus**

Prometheus configuration example (`prometheus.yml`):

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'supernova'
    static_configs:
      - targets: ['localhost:9090']
    metrics_path: /metrics
```

2. **Grafana Dashboards**

Import the provided Supernova dashboards to Grafana:

- Blockchain Overview Dashboard (ID: 12345)
- Node Performance Dashboard (ID: 12346)
- Network Overview Dashboard (ID: 12347)

3. **Setting Up Alerts**

Configure AlertManager rules for critical conditions:

```yaml
groups:
- name: supernova
  rules:
  - alert: NodeDown
    expr: up{job="supernova"} == 0
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "Supernova node is down"
      description: "{{ $labels.instance }} has been down for more than 5 minutes."

  - alert: HighMemoryUsage
    expr: process_resident_memory_bytes{job="supernova"} / 1024 / 1024 > 7168
    for: 10m
    labels:
      severity: warning
    annotations:
      summary: "High memory usage detected"
      description: "{{ $labels.instance }} has used more than 7GB of memory for 10 minutes."

  - alert: BlockchainStalled
    expr: increase(supernova_block_height[1h]) == 0
    for: 30m
    labels:
      severity: critical
    annotations:
      summary: "Blockchain is stalled"
      description: "No new blocks have been observed in the last 30 minutes."
```

### Log Management

1. **Configure Log Rotation**

```bash
sudo nano /etc/logrotate.d/supernova
```

Add the following:

```
/var/log/supernova/*.log {
    daily
    rotate 14
    compress
    delaycompress
    notifempty
    create 0640 supernova supernova
    sharedscripts
    postrotate
        systemctl reload supernova
    endscript
}
```

2. **Centralized Logging**

For multi-node setups, consider using a centralized logging solution like ELK Stack (Elasticsearch, Logstash, Kibana) or Graylog.

## Backup and Recovery

### Automated Backup Configuration

1. **Create Backup Script**

```bash
#!/bin/bash
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
BACKUP_DIR="/mnt/backups/supernova"
DATA_DIR="/home/supernova/data"
RETENTION_DAYS=30

# Create backup directory if it doesn't exist
mkdir -p $BACKUP_DIR

# Create backup
tar -czf "$BACKUP_DIR/supernova_backup_$TIMESTAMP.tar.gz" -C $DATA_DIR .

# Remove old backups
find $BACKUP_DIR -name "supernova_backup_*.tar.gz" -type f -mtime +$RETENTION_DAYS -delete
```

2. **Create Cron Job**

```bash
crontab -e
```

Add the following line to run daily at 2 AM:

```
0 2 * * * /path/to/backup_script.sh
```

### Recovery Procedure

1. **Stop the Node**

```bash
sudo systemctl stop supernova
```

2. **Restore from Backup**

```bash
# Remove existing data
rm -rf /home/supernova/data/*

# Restore from backup
tar -xzf /mnt/backups/supernova/supernova_backup_TIMESTAMP.tar.gz -C /home/supernova/data/
```

3. **Restart the Node**

```bash
sudo systemctl start supernova
```

## Security Best Practices

### Network Security

1. **Firewall Configuration**
   - Only expose necessary ports
   - Restrict RPC access to trusted IPs
   - Use VPN for remote management

2. **Network Segregation**
   - Use separate networks for public and private services
   - Implement proper security groups/firewall rules

### System Security

1. **Regular Updates**
   - Keep the operating system and dependencies updated
   - Apply security patches promptly

2. **Minimal Permissions**
   - Run the Supernova node as a non-root user
   - Use restricted permissions for configuration and data files

3. **Secure Configuration**
   - Use strong authentication for RPC access
   - Disable unnecessary services and features

### Operational Security

1. **Regular Security Audits**
   - Conduct periodic security audits
   - Monitor for suspicious activities

2. **Key Management**
   - Use proper key management practices
   - Rotate credentials periodically

3. **Access Control**
   - Implement multi-factor authentication for administrative access
   - Use the principle of least privilege

## Troubleshooting

### Common Issues and Solutions

1. **Node Not Syncing**
   - Check network connectivity
   - Verify firewall settings
   - Ensure sufficient disk space

2. **High Resource Usage**
   - Check for abnormal activity
   - Consider increasing resource allocation
   - Optimize configuration parameters

3. **Database Corruption**
   - Stop the node
   - Restore from a backup
   - If no backup is available, resync from scratch

### Diagnostic Commands

```bash
# Check node status
curl -s http://localhost:9332/api/v1/node/status | jq

# View logs
journalctl -u supernova -f

# Check resource usage
top -c -p $(pgrep -f supernova)

# Check network connections
netstat -tuln | grep -E '9333|9332'

# Verify database integrity
supernova check-db --path /home/supernova/data
```

## Upgrade Procedures

### Docker Upgrade

1. **Pull the Latest Image**

```bash
docker pull supernova:latest
```

2. **Stop the Current Container**

```bash
docker stop supernova-node
docker rm supernova-node
```

3. **Start a New Container with the Latest Image**

```bash
docker run -d --name supernova-node \
  -p 9333:9333 -p 9332:9332 -p 9090:9090 \
  -v $(pwd)/data:/home/supernova/data \
  -v $(pwd)/config/node.toml:/home/supernova/config/default.toml \
  supernova:latest
```

### Kubernetes Upgrade

1. **Update the Helm Repository**

```bash
helm repo update
```

2. **Upgrade the Release**

```bash
helm upgrade supernova supernova/supernova -f values.yaml
```

### Bare Metal Upgrade

1. **Stop the Service**

```bash
sudo systemctl stop supernova
```

2. **Backup Data**

```bash
# Create a backup
tar -czf ~/supernova_backup_before_upgrade.tar.gz -C /home/supernova/data .
```

3. **Update the Code**

```bash
cd supernova
git pull
cargo build --release
```

4. **Restart the Service**

```bash
sudo systemctl start supernova
```

5. **Monitor Logs for Issues**

```bash
journalctl -u supernova -f
```

---

This deployment guide provides a comprehensive overview of various deployment options and best practices for the Supernova blockchain. For additional support, please refer to the project documentation or contact the development team. 