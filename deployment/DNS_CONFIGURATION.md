# DNS Configuration for Supernova Testnet

## Required DNS Records

Add these A records to your domain provider for `supernovanetwork.xyz`:

### Testnet Subdomain Records
```
Type    Name                    Value           TTL     Purpose
----------------------------------------------------------------------
A       testnet                 YOUR_VPS_IP     300     Main testnet endpoint
A       api.testnet            YOUR_VPS_IP     300     API service
A       faucet.testnet         YOUR_VPS_IP     300     Token faucet
A       wallet.testnet         YOUR_VPS_IP     300     Web wallet
A       dashboard.testnet      YOUR_VPS_IP     300     Monitoring dashboard
A       grafana.testnet        YOUR_VPS_IP     300     Metrics dashboard (optional)
```

### Example with IP 123.45.67.89:
```
A       testnet                 123.45.67.89    300
A       api.testnet            123.45.67.89    300
A       faucet.testnet         123.45.67.89    300
A       wallet.testnet         123.45.67.89    300
A       dashboard.testnet      123.45.67.89    300
A       grafana.testnet        123.45.67.89    300
```

## How to Add These Records

### Option 1: Through Your Domain Registrar
1. Log into your domain registrar (GoDaddy, Namecheap, etc.)
2. Find DNS Management or DNS Settings
3. Add each A record listed above
4. Save changes

### Option 2: Through Netlify DNS (if using Netlify DNS)
1. Go to Netlify Dashboard
2. Select your domain
3. Go to DNS settings
4. Add A records for each subdomain

## Verification

After adding DNS records, verify they're working:

```bash
# Check each subdomain
dig testnet.supernovanetwork.xyz
dig api.testnet.supernovanetwork.xyz
dig faucet.testnet.supernovanetwork.xyz
dig wallet.testnet.supernovanetwork.xyz

# Or use nslookup
nslookup testnet.supernovanetwork.xyz
```

## SSL Certificate Coverage

The deployment script will automatically obtain SSL certificates for all these subdomains using Let's Encrypt.

## Final URLs After Deployment

- **Dashboard**: https://testnet.supernovanetwork.xyz
- **API**: https://api.testnet.supernovanetwork.xyz
- **Faucet**: https://faucet.testnet.supernovanetwork.xyz
- **Wallet**: https://wallet.testnet.supernovanetwork.xyz
- **Monitoring**: https://dashboard.testnet.supernovanetwork.xyz
- **Metrics**: https://grafana.testnet.supernovanetwork.xyz

## Notes

- DNS propagation can take 5-30 minutes
- The deployment script handles all nginx routing automatically
- All subdomains point to the same VPS IP
- Nginx will route traffic to the correct service based on the subdomain 