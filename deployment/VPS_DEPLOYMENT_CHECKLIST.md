# VPS Deployment Checklist for Supernova Testnet

## Pre-Deployment Requirements

### 1. Cloud VPS Setup
- [ ] **Choose VPS Provider**:
  - Recommended: DigitalOcean, Linode, Vultr, AWS EC2
  - Alternative: Hetzner, OVH, Google Cloud
  
- [ ] **VPS Specifications** (Minimum):
  - CPU: 4 vCPUs
  - RAM: 8GB
  - Storage: 100GB SSD
  - Bandwidth: 1TB/month
  - OS: Ubuntu 22.04 LTS
  
- [ ] **Estimated Costs**:
  - DigitalOcean: ~$48/month (4vCPU, 8GB RAM)
  - Linode: ~$48/month (4vCPU, 8GB RAM)
  - AWS EC2: ~$70/month (t3.xlarge)

### 2. Domain Configuration
- [ ] **DNS Records** (add to your domain provider):
  ```
  Type    Name                    Value           TTL
  A       testnet                 YOUR_VPS_IP     300
  A       api.testnet            YOUR_VPS_IP     300
  A       faucet.testnet         YOUR_VPS_IP     300
  A       dashboard.testnet      YOUR_VPS_IP     300
  A       grafana.testnet        YOUR_VPS_IP     300
  ```

### 3. Security Preparation
- [ ] Generate strong root password
- [ ] Prepare SSH key pair
- [ ] Have email ready for SSL certificates

## Deployment Steps

### Step 1: Initial VPS Setup
```bash
# 1. SSH into your VPS
ssh root@YOUR_VPS_IP

# 2. Update system
apt update && apt upgrade -y

# 3. Create swap file (recommended)
fallocate -l 4G /swapfile
chmod 600 /swapfile
mkswap /swapfile
swapon /swapfile
echo '/swapfile none swap sw 0 0' >> /etc/fstab

# 4. Set timezone
timedatectl set-timezone UTC
```

### Step 2: Run Deployment Script
```bash
# 1. Download deployment script
wget https://raw.githubusercontent.com/mjohnson518/supernova/main/deployment/scripts/deploy-testnet.sh

# 2. Make executable
chmod +x deploy-testnet.sh

# 3. Edit configuration (update domain and email)
nano deploy-testnet.sh
# Change DOMAIN and EMAIL variables

# 4. Run deployment
./deploy-testnet.sh
```

### Step 3: Post-Deployment Configuration
- [ ] Change default passwords
- [ ] Configure backup storage
- [ ] Set up monitoring alerts
- [ ] Test all endpoints

## Quick Deployment (One-Liner)

For experienced users:
```bash
curl -sSL https://raw.githubusercontent.com/mjohnson518/supernova/main/deployment/scripts/deploy-testnet.sh | \
  DOMAIN=testnet.supernovanetwork.xyz \
  EMAIL=admin@supernovanetwork.xyz \
  bash
```

## Monitoring & Maintenance

### Daily Tasks
- [ ] Check dashboard: https://testnet.supernovanetwork.xyz
- [ ] Monitor disk space: `df -h`
- [ ] Check logs: `docker compose logs -f`

### Weekly Tasks
- [ ] Review Grafana metrics
- [ ] Check backup integrity
- [ ] Update system packages
- [ ] Review security logs

### Monthly Tasks
- [ ] Full system backup
- [ ] Security audit
- [ ] Performance optimization
- [ ] Cost review

## Troubleshooting

### If deployment fails:
1. Check logs: `/var/log/supernova/deployment.log`
2. Verify DNS propagation: `dig testnet.yourdomain.xyz`
3. Check firewall: `ufw status`
4. Verify Docker: `docker ps`

### Common Issues:

**SSL Certificate Error**:
```bash
# Manually obtain certificate
certbot certonly --standalone -d testnet.yourdomain.xyz
```

**Port Already in Use**:
```bash
# Find process using port
lsof -i :8333
# Kill process if needed
kill -9 PID
```

**Out of Memory**:
```bash
# Check memory usage
free -h
# Restart services
docker compose restart
```

## Scaling Considerations

### When to Upgrade:
- CPU usage consistently >80%
- Memory usage >90%
- Disk usage >80%
- Network latency issues

### Upgrade Options:
1. **Vertical Scaling**: Upgrade VPS plan
2. **Horizontal Scaling**: Add more nodes
3. **CDN Integration**: For dashboard/API
4. **Load Balancing**: For high traffic

## Cost Optimization

### Tips to Reduce Costs:
1. Use reserved instances (AWS/GCP)
2. Enable auto-scaling during peak times
3. Compress logs and backups
4. Use object storage for backups
5. Monitor and eliminate unused resources

### Estimated Monthly Costs:
- VPS: $48-100
- Backup Storage: $5-20
- Bandwidth Overage: $0-50
- **Total**: $53-170/month

## Security Hardening

### Additional Security Steps:
```bash
# 1. Disable root SSH
sed -i 's/PermitRootLogin yes/PermitRootLogin no/' /etc/ssh/sshd_config

# 2. Install fail2ban
apt install fail2ban -y

# 3. Configure automatic updates
apt install unattended-upgrades -y
dpkg-reconfigure -plow unattended-upgrades

# 4. Set up UFW firewall
ufw default deny incoming
ufw default allow outgoing
ufw allow 22,80,443,8333,8343,9735/tcp
ufw enable
```

## Support Resources

- **Documentation**: https://docs.supernovanetwork.xyz
- **Discord**: https://discord.gg/supernova
- **GitHub Issues**: https://github.com/mjohnson518/supernova/issues
- **Email Support**: support@supernovanetwork.xyz

---

## Quick Reference

### Essential Commands:
```bash
# View status
docker compose ps

# Restart all services
docker compose restart

# View logs
docker compose logs -f [service-name]

# Backup blockchain
docker exec supernova-bootstrap-1 supernova-node backup

# Check node sync
docker exec supernova-bootstrap-1 supernova-node status
```

### Service URLs:
- Dashboard: https://testnet.yourdomain.xyz
- API: https://api.testnet.yourdomain.xyz
- Faucet: https://faucet.testnet.yourdomain.xyz
- Grafana: https://grafana.testnet.yourdomain.xyz

### Default Ports:
- P2P: 8333, 8343
- RPC: 8332, 8342
- Lightning: 9735
- HTTP(S): 80, 443

---

**Ready to launch?** Follow this checklist and you'll have a production-ready testnet in under 30 minutes! 🚀 