#!/bin/bash

# NOTICE: Testnet web files have been migrated to the supernova-web repository
# This script is deprecated and will be removed in a future version
# Please use the deployment process in the supernova-web repository instead

echo "⚠️  WARNING: Testnet web files have been moved to the supernova-web repository"
echo "This deployment script is deprecated."
echo "Please clone https://github.com/Carbon-Twelve-C12/supernova-web and use its deployment process."
echo ""
echo "To deploy testnet web interfaces:"
echo "1. Clone the supernova-web repository"
echo "2. Follow the deployment instructions in that repository"
echo ""
echo "This script will be removed in a future version."
exit 1

# Deploy Supernova Testnet Subdomain Web Files
set -e

echo "🚀 Deploying Supernova Testnet Subdomain Web Files..."

# Set TESTNET_SERVER in your environment before running this script
# Example: export TESTNET_SERVER="user@your-server-ip"
SERVER=${TESTNET_SERVER:-"supernova@testnet.supernovanetwork.xyz"}

# Create directories on server
echo "📁 Creating directories on server..."
ssh -t $SERVER "sudo mkdir -p /var/www/html/testnet/{explorer,status,faucet,api} && sudo chown -R supernova:supernova /var/www/html/testnet"

# Deploy Explorer
echo "📊 Deploying Explorer..."
scp -r deployments/testnet/web/explorer/* $SERVER:/var/www/html/testnet/explorer/

# Deploy Status Dashboard
echo "📈 Deploying Status Dashboard..."
scp -r deployments/testnet/web/status/* $SERVER:/var/www/html/testnet/status/

# Deploy Faucet
echo "💰 Deploying Faucet..."
scp -r deployments/testnet/web/faucet/* $SERVER:/var/www/html/testnet/faucet/

# Create API landing page
echo "🔌 Creating API documentation landing page..."
cat << 'HTML' > /tmp/api_index.html
<!DOCTYPE html>
<html>
<head>
    <title>Supernova Testnet API</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        code { background: #f4f4f4; padding: 2px 5px; }
    </style>
</head>
<body>
    <h1>Supernova Testnet API</h1>
    <p>API Base URL: <code>http://api.testnet.supernovanetwork.xyz/</code></p>
    <h2>Example Endpoints:</h2>
    <ul>
        <li><a href="/blockchain/info">/blockchain/info</a> - Get blockchain information</li>
        <li><a href="/node/status">/node/status</a> - Get node status</li>
        <li><a href="/network/peers">/network/peers</a> - List connected peers</li>
    </ul>
    <p>All endpoints are prefixed with <code>/api/v1</code> in the actual implementation.</p>
</body>
</html>
HTML
scp /tmp/api_index.html $SERVER:/tmp/api_index.html
ssh -t $SERVER "sudo mv /tmp/api_index.html /var/www/html/testnet/api/index.html"
rm /tmp/api_index.html

# Set permissions
echo "🔐 Setting permissions..."
ssh -t $SERVER "sudo chown -R www-data:www-data /var/www/html/testnet && sudo chmod -R 755 /var/www/html/testnet"

# Ensure nginx config exists
echo "📝 Deploying nginx configuration..."
scp nginx_testnet_config.conf $SERVER:/tmp/testnet.conf
ssh -t $SERVER "sudo cp /tmp/testnet.conf /etc/nginx/sites-available/testnet && sudo ln -sf /etc/nginx/sites-available/testnet /etc/nginx/sites-enabled/ && sudo nginx -t && sudo systemctl reload nginx"

echo "
✅ Deployment complete!

Test the subdomains:
- http://explorer.testnet.supernovanetwork.xyz
- http://status.testnet.supernovanetwork.xyz
- http://faucet.testnet.supernovanetwork.xyz
- http://api.testnet.supernovanetwork.xyz
" 