#!/bin/bash

# Deploy Supernova testnet services

# Set TESTNET_SERVER in your environment before running
SERVER=${TESTNET_SERVER:-"supernova@testnet.supernovanetwork.xyz"}

echo "=== Deploying Supernova Testnet Services ==="

# Copy nginx configuration
echo "1. Copying nginx configuration..."
scp nginx_testnet_config.conf $SERVER:/tmp/

# Copy web files
echo "2. Copying web files..."
scp -r deployments/testnet/web/faucet $SERVER:/tmp/
scp -r deployments/testnet/web/explorer $SERVER:/tmp/ 2>/dev/null || echo "Explorer not found, skipping..."
scp -r deployments/testnet/web/status $SERVER:/tmp/ 2>/dev/null || echo "Status page not found, skipping..."

# SSH and set up everything
ssh $SERVER << 'EOF'
    echo "3. Setting up nginx configuration..."
    
    # Create nginx config
    sudo cp /tmp/nginx_testnet_config.conf /etc/nginx/sites-available/testnet-services
    sudo ln -sf /etc/nginx/sites-available/testnet-services /etc/nginx/sites-enabled/
    
    echo "4. Creating directory structure..."
    
    # Create directories for each service
    sudo mkdir -p /var/www/html/testnet/{faucet,explorer,status,dashboard,wallet}
    
    # Copy faucet files if they exist
    if [ -d /tmp/faucet ]; then
        sudo cp -r /tmp/faucet/* /var/www/html/testnet/faucet/
    fi
    
    # Create placeholder pages for services not yet deployed
    
    # Explorer placeholder
    if [ ! -f /var/www/html/testnet/explorer/index.html ]; then
        echo '<!DOCTYPE html>
<html>
<head>
    <title>Supernova Block Explorer</title>
    <style>
        body { 
            font-family: Arial, sans-serif; 
            background: #1a1a2e; 
            color: #fff;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
        }
        .container {
            text-align: center;
        }
        h1 { color: #00ff88; }
        a { color: #00ccff; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Block Explorer Coming Soon</h1>
        <p>The Supernova block explorer is under development.</p>
        <p>For now, use the <a href="http://api.testnet.supernovanetwork.xyz/blockchain/info">API</a> to explore blocks.</p>
        <br>
        <a href="http://testnet.supernovanetwork.xyz">Back to Testnet Home</a>
    </div>
</body>
</html>' | sudo tee /var/www/html/testnet/explorer/index.html > /dev/null
    fi
    
    # Status placeholder
    if [ ! -f /var/www/html/testnet/status/index.html ]; then
        echo '<!DOCTYPE html>
<html>
<head>
    <title>Supernova Network Status</title>
    <style>
        body { 
            font-family: Arial, sans-serif; 
            background: #1a1a2e; 
            color: #fff;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
        }
        .container {
            text-align: center;
        }
        h1 { color: #00ff88; }
        a { color: #00ccff; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Network Status Dashboard</h1>
        <p>Real-time network monitoring coming soon.</p>
        <p>Current node status: <a href="http://api.testnet.supernovanetwork.xyz/node/info">API Status</a></p>
        <br>
        <a href="http://testnet.supernovanetwork.xyz">Back to Testnet Home</a>
    </div>
</body>
</html>' | sudo tee /var/www/html/testnet/status/index.html > /dev/null
    fi
    
    # Set permissions
    echo "5. Setting permissions..."
    sudo chown -R www-data:www-data /var/www/html/testnet/
    sudo chmod -R 755 /var/www/html/testnet/
    
    # Test nginx config
    echo "6. Testing nginx configuration..."
    sudo nginx -t
    
    # Reload nginx
    echo "7. Reloading nginx..."
    sudo systemctl reload nginx
    
    echo "Deployment complete!"
EOF

echo ""
echo "=== Deployment Summary ==="
echo "API: http://api.testnet.supernovanetwork.xyz"
echo "Faucet: http://faucet.testnet.supernovanetwork.xyz"
echo "Explorer: http://explorer.testnet.supernovanetwork.xyz"
echo "Status: http://status.testnet.supernovanetwork.xyz"
echo ""
echo "Test with: curl http://api.testnet.supernovanetwork.xyz/node/info" 