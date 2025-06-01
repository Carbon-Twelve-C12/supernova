#!/bin/bash
# Supernova Testnet Deployment Script
# Deploy complete testnet infrastructure on cloud VPS

set -e

# Configuration
DOMAIN="testnet.supernovanetwork.xyz"
EMAIL="admin@supernovanetwork.xyz"
INSTALL_DIR="/opt/supernova"
DATA_DIR="/var/lib/supernova"
LOG_DIR="/var/log/supernova"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging function
log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1" >&2
}

warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

# Check if running as root
check_root() {
    if [[ $EUID -ne 0 ]]; then
        error "This script must be run as root"
        exit 1
    fi
}

# Update system
update_system() {
    log "Updating system packages..."
    apt-get update -y
    apt-get upgrade -y
    apt-get install -y \
        curl \
        wget \
        git \
        build-essential \
        pkg-config \
        libssl-dev \
        ca-certificates \
        gnupg \
        lsb-release \
        ufw \
        fail2ban \
        htop \
        iotop \
        nethogs
}

# Install Docker
install_docker() {
    log "Installing Docker..."
    
    # Remove old versions
    apt-get remove -y docker docker-engine docker.io containerd runc || true
    
    # Add Docker's official GPG key
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg
    
    # Add Docker repository
    echo \
        "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu \
        $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null
    
    # Install Docker
    apt-get update -y
    apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin
    
    # Enable and start Docker
    systemctl enable docker
    systemctl start docker
    
    log "Docker installed successfully"
}

# Install Nginx and Certbot
install_nginx_certbot() {
    log "Installing Nginx and Certbot..."
    
    apt-get install -y nginx certbot python3-certbot-nginx
    
    # Stop nginx temporarily
    systemctl stop nginx
}

# Configure firewall
configure_firewall() {
    log "Configuring firewall..."
    
    # Allow SSH
    ufw allow 22/tcp
    
    # Allow HTTP and HTTPS
    ufw allow 80/tcp
    ufw allow 443/tcp
    
    # Allow blockchain P2P ports
    ufw allow 8333/tcp
    ufw allow 8343/tcp
    
    # Allow Lightning Network
    ufw allow 9735/tcp
    
    # Enable firewall
    ufw --force enable
    
    log "Firewall configured"
}

# Setup directories
setup_directories() {
    log "Setting up directories..."
    
    mkdir -p $INSTALL_DIR
    mkdir -p $DATA_DIR/{blockchain,lightning,oracle,faucet}
    mkdir -p $LOG_DIR
    mkdir -p $INSTALL_DIR/deployment/{docker,nginx,prometheus,grafana}
    
    # Set permissions
    chmod -R 755 $INSTALL_DIR
    chmod -R 750 $DATA_DIR
    chmod -R 750 $LOG_DIR
}

# Clone repository
clone_repository() {
    log "Cloning Supernova repository..."
    
    cd $INSTALL_DIR
    if [ -d "supernova" ]; then
        cd supernova
        git pull origin main
    else
        git clone https://github.com/supernova-network/supernova.git
        cd supernova
    fi
}

# Setup SSL certificates
setup_ssl() {
    log "Setting up SSL certificates..."
    
    # Get certificates for all subdomains
    certbot certonly --standalone \
        -d $DOMAIN \
        -d api.$DOMAIN \
        -d faucet.$DOMAIN \
        -d wallet.$DOMAIN \
        -d dashboard.$DOMAIN \
        -d grafana.$DOMAIN \
        --non-interactive \
        --agree-tos \
        --email $EMAIL
    
    # Create SSL directory for Docker
    mkdir -p $INSTALL_DIR/deployment/nginx/ssl
    
    # Copy certificates
    cp /etc/letsencrypt/live/$DOMAIN/fullchain.pem $INSTALL_DIR/deployment/nginx/ssl/
    cp /etc/letsencrypt/live/$DOMAIN/privkey.pem $INSTALL_DIR/deployment/nginx/ssl/
    
    log "SSL certificates configured"
}

# Configure Nginx
configure_nginx() {
    log "Configuring Nginx..."
    
    # Create Nginx configuration
    cat > $INSTALL_DIR/deployment/nginx/nginx.conf << 'EOF'
user nginx;
worker_processes auto;
error_log /var/log/nginx/error.log warn;
pid /var/run/nginx.pid;

events {
    worker_connections 1024;
}

http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;
    
    log_format main '$remote_addr - $remote_user [$time_local] "$request" '
                    '$status $body_bytes_sent "$http_referer" '
                    '"$http_user_agent" "$http_x_forwarded_for"';
    
    access_log /var/log/nginx/access.log main;
    
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 65;
    types_hash_max_size 2048;
    
    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "no-referrer-when-downgrade" always;
    
    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api_limit:10m rate=10r/s;
    limit_req_zone $binary_remote_addr zone=faucet_limit:10m rate=1r/m;
    
    include /etc/nginx/sites-enabled/*;
}
EOF

    # Create site configurations
    mkdir -p $INSTALL_DIR/deployment/nginx/sites-enabled
    
    # Main testnet site
    cat > $INSTALL_DIR/deployment/nginx/sites-enabled/testnet.conf << EOF
server {
    listen 80;
    server_name $DOMAIN;
    return 301 https://\$server_name\$request_uri;
}

server {
    listen 443 ssl http2;
    server_name $DOMAIN;
    
    ssl_certificate /etc/nginx/ssl/fullchain.pem;
    ssl_certificate_key /etc/nginx/ssl/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    
    location / {
        proxy_pass http://dashboard:3000;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        
        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}

# API subdomain
server {
    listen 443 ssl http2;
    server_name api.$DOMAIN;
    
    ssl_certificate /etc/nginx/ssl/fullchain.pem;
    ssl_certificate_key /etc/nginx/ssl/privkey.pem;
    
    location / {
        limit_req zone=api_limit burst=20 nodelay;
        
        proxy_pass http://api:8080;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}

# Faucet subdomain
server {
    listen 443 ssl http2;
    server_name faucet.$DOMAIN;
    
    ssl_certificate /etc/nginx/ssl/fullchain.pem;
    ssl_certificate_key /etc/nginx/ssl/privkey.pem;
    
    location / {
        limit_req zone=faucet_limit burst=5 nodelay;
        
        proxy_pass http://faucet:4000;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}

# Grafana subdomain
server {
    listen 443 ssl http2;
    server_name grafana.$DOMAIN;
    
    ssl_certificate /etc/nginx/ssl/fullchain.pem;
    ssl_certificate_key /etc/nginx/ssl/privkey.pem;
    
    location / {
        proxy_pass http://grafana:3000;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}
EOF

    log "Nginx configured"
}

# Setup monitoring
setup_monitoring() {
    log "Setting up monitoring..."
    
    # Prometheus configuration
    cat > $INSTALL_DIR/deployment/prometheus/prometheus.yml << 'EOF'
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'supernova-nodes'
    static_configs:
      - targets: ['bootstrap-node-1:9100', 'bootstrap-node-2:9100']
        labels:
          group: 'bootstrap'
      
  - job_name: 'supernova-api'
    static_configs:
      - targets: ['api:9100']
      
  - job_name: 'supernova-oracles'
    static_configs:
      - targets: ['oracle-carbon:9100', 'oracle-renewable:9100']
        labels:
          group: 'oracle'
EOF

    # Create Grafana dashboards directory
    mkdir -p $INSTALL_DIR/deployment/grafana/dashboards
    
    log "Monitoring configured"
}

# Deploy with Docker Compose
deploy_docker_compose() {
    log "Deploying with Docker Compose..."
    
    cd $INSTALL_DIR/supernova
    
    # Copy deployment files
    cp -r deployment/* $INSTALL_DIR/deployment/
    
    # Build and start services
    cd $INSTALL_DIR/deployment/docker
    docker compose up -d --build
    
    log "Docker Compose deployment started"
}

# Setup auto-renewal for SSL
setup_ssl_renewal() {
    log "Setting up SSL auto-renewal..."
    
    # Create renewal script
    cat > /etc/cron.daily/supernova-ssl-renew << 'EOF'
#!/bin/bash
certbot renew --quiet --post-hook "docker restart supernova-nginx"

# Copy renewed certificates
cp /etc/letsencrypt/live/testnet.supernovanetwork.xyz/fullchain.pem /opt/supernova/deployment/nginx/ssl/
cp /etc/letsencrypt/live/testnet.supernovanetwork.xyz/privkey.pem /opt/supernova/deployment/nginx/ssl/
EOF
    
    chmod +x /etc/cron.daily/supernova-ssl-renew
    
    log "SSL auto-renewal configured"
}

# Setup backup
setup_backup() {
    log "Setting up automated backups..."
    
    # Create backup script
    cat > /usr/local/bin/supernova-backup.sh << 'EOF'
#!/bin/bash
BACKUP_DIR="/backup/supernova"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR

# Backup blockchain data
docker exec supernova-bootstrap-1 supernova-node backup --output /tmp/backup.tar.gz
docker cp supernova-bootstrap-1:/tmp/backup.tar.gz $BACKUP_DIR/blockchain_$DATE.tar.gz

# Backup volumes
docker run --rm -v bootstrap1-data:/data -v $BACKUP_DIR:/backup alpine tar czf /backup/bootstrap1_$DATE.tar.gz -C /data .

# Keep only last 7 days of backups
find $BACKUP_DIR -name "*.tar.gz" -mtime +7 -delete
EOF
    
    chmod +x /usr/local/bin/supernova-backup.sh
    
    # Add to crontab
    echo "0 3 * * * /usr/local/bin/supernova-backup.sh" | crontab -
    
    log "Automated backups configured"
}

# Health check
setup_health_check() {
    log "Setting up health monitoring..."
    
    # Create health check script
    cat > /usr/local/bin/supernova-health-check.sh << 'EOF'
#!/bin/bash
# Check if all services are running
SERVICES=("supernova-bootstrap-1" "supernova-bootstrap-2" "supernova-dashboard" "supernova-api" "supernova-faucet")

for service in "${SERVICES[@]}"; do
    if ! docker ps | grep -q $service; then
        echo "Service $service is not running. Attempting restart..."
        docker start $service
        
        # Send alert (configure your alert mechanism)
        # curl -X POST https://alerts.example.com/webhook -d "Service $service was down and restarted"
    fi
done

# Check disk space
DISK_USAGE=$(df -h / | awk 'NR==2 {print $5}' | sed 's/%//')
if [ $DISK_USAGE -gt 80 ]; then
    echo "Warning: Disk usage is at $DISK_USAGE%"
    # Send disk space alert
fi
EOF
    
    chmod +x /usr/local/bin/supernova-health-check.sh
    
    # Add to crontab (every 5 minutes)
    (crontab -l 2>/dev/null; echo "*/5 * * * * /usr/local/bin/supernova-health-check.sh") | crontab -
    
    log "Health monitoring configured"
}

# Final setup
final_setup() {
    log "Performing final setup..."
    
    # Create systemd service for startup
    cat > /etc/systemd/system/supernova-testnet.service << EOF
[Unit]
Description=Supernova Testnet
Requires=docker.service
After=docker.service

[Service]
Type=oneshot
RemainAfterExit=yes
WorkingDirectory=$INSTALL_DIR/deployment/docker
ExecStart=/usr/bin/docker compose up -d
ExecStop=/usr/bin/docker compose down
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF
    
    systemctl enable supernova-testnet.service
    
    # Start Nginx
    systemctl start nginx
    systemctl enable nginx
    
    log "Final setup complete"
}

# Display information
display_info() {
    echo
    echo "=========================================="
    echo -e "${GREEN}Supernova Testnet Deployment Complete!${NC}"
    echo "=========================================="
    echo
    echo "Access URLs:"
    echo "  Dashboard: https://$DOMAIN"
    echo "  API: https://api.$DOMAIN"
    echo "  Faucet: https://faucet.$DOMAIN"
    echo "  Grafana: https://grafana.$DOMAIN"
    echo
    echo "P2P Endpoints:"
    echo "  Bootstrap Node 1: $DOMAIN:8333"
    echo "  Bootstrap Node 2: $DOMAIN:8343"
    echo "  Lightning Network: $DOMAIN:9735"
    echo
    echo "Management Commands:"
    echo "  View logs: docker compose -f $INSTALL_DIR/deployment/docker/docker-compose.yml logs -f"
    echo "  Stop services: docker compose -f $INSTALL_DIR/deployment/docker/docker-compose.yml down"
    echo "  Start services: docker compose -f $INSTALL_DIR/deployment/docker/docker-compose.yml up -d"
    echo "  View node status: docker exec supernova-bootstrap-1 supernova-node status"
    echo
    echo "Default Grafana login: admin / supernova-testnet"
    echo
    echo "For your friends to join the network, they should:"
    echo "  1. Visit https://faucet.$DOMAIN to get test NOVA"
    echo "  2. Connect their node to: $DOMAIN:8333"
    echo "  3. View network stats at: https://$DOMAIN"
    echo
    warning "Remember to:"
    warning "  - Change default passwords"
    warning "  - Monitor disk space"
    warning "  - Check logs regularly"
    warning "  - Keep the system updated"
    echo
}

# Main execution
main() {
    log "Starting Supernova Testnet deployment..."
    
    check_root
    update_system
    install_docker
    install_nginx_certbot
    configure_firewall
    setup_directories
    clone_repository
    setup_ssl
    configure_nginx
    setup_monitoring
    deploy_docker_compose
    setup_ssl_renewal
    setup_backup
    setup_health_check
    final_setup
    
    display_info
    
    log "Deployment completed successfully!"
}

# Run main function
main "$@" 