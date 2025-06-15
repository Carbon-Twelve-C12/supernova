#!/bin/bash

# Deploy testnet web files to server

SERVER="supernova@146.190.225.136"
WEB_ROOT="/var/www/html"

echo "Deploying testnet web files..."

# Copy the updated index.html
echo "Copying index.html..."
scp deployments/testnet/web/landing/index.html $SERVER:/tmp/index.html

# SSH into server and move files with sudo
ssh $SERVER << 'EOF'
    echo "Moving files to web root..."
    sudo cp /tmp/index.html /var/www/html/index.html
    
    echo "Setting permissions..."
    sudo chown www-data:www-data /var/www/html/index.html
    sudo chmod 644 /var/www/html/index.html
    
    echo "Files deployed successfully!"
EOF

echo "Deployment complete!"
echo "Test the updated page at: http://146.190.225.136/" 