#!/bin/bash

# Test script for Supernova testnet web interfaces
# This script verifies that all web interfaces can properly connect to the blockchain API

set -e

echo "=== Supernova Testnet Web Interface Test ==="
echo

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# API base URL
API_BASE="http://testnet.supernovanetwork.xyz:8332/api/v1"

# Function to test API endpoint
test_endpoint() {
    local endpoint=$1
    local description=$2
    
    echo -n "Testing $description... "
    
    if curl -s -f -m 10 "$API_BASE$endpoint" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ OK${NC}"
        return 0
    else
        echo -e "${RED}✗ FAILED${NC}"
        return 1
    fi
}

# Function to test web interface
test_web_interface() {
    local port=$1
    local name=$2
    local path=${3:-"/"}
    
    echo -n "Testing $name interface (port $port)... "
    
    if curl -s -f -m 10 "http://testnet.supernovanetwork.xyz:$port$path" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ OK${NC}"
        return 0
    else
        echo -e "${YELLOW}⚠ Not accessible (may not be deployed)${NC}"
        return 1
    fi
}

# Test blockchain API endpoints
echo "1. Testing Blockchain API Endpoints"
echo "-----------------------------------"

test_endpoint "/blockchain/info" "Blockchain Info"
test_endpoint "/blockchain/stats" "Blockchain Stats"
test_endpoint "/mempool/info" "Mempool Info"
test_endpoint "/network/info" "Network Info"
test_endpoint "/node/info" "Node Info"
test_endpoint "/wallet/info" "Wallet Info"
test_endpoint "/wallet/balance" "Wallet Balance"
test_endpoint "/faucet/status" "Faucet Status"

echo

# Test web interfaces
echo "2. Testing Web Interface Availability"
echo "------------------------------------"

test_web_interface 3001 "Block Explorer"
test_web_interface 3002 "Faucet"
test_web_interface 3003 "Wallet"
test_web_interface 3004 "Status Dashboard"

echo

# Test specific functionality
echo "3. Testing Interface Functionality"
echo "---------------------------------"

# Test Explorer API proxy
echo -n "Testing Explorer API proxy... "
if curl -s -f -m 10 "http://testnet.supernovanetwork.xyz:3001/api/v1/blockchain/info" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${YELLOW}⚠ Explorer API proxy not working${NC}"
fi

# Test Faucet recent transactions
echo -n "Testing Faucet recent transactions... "
FAUCET_RESPONSE=$(curl -s -m 10 "$API_BASE/faucet/transactions" 2>/dev/null)
if [[ $FAUCET_RESPONSE == *"transactions"* ]]; then
    echo -e "${GREEN}✓ OK${NC}"
else
    echo -e "${RED}✗ FAILED${NC}"
fi

echo

# Summary
echo "4. Connection Test Summary"
echo "-------------------------"

# Test if node is actually running
echo -n "Checking if testnet node is running... "
if curl -s -f -m 5 "$API_BASE/blockchain/info" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Node is running${NC}"
    
    # Get some basic info
    INFO=$(curl -s "$API_BASE/blockchain/info" 2>/dev/null)
    HEIGHT=$(echo "$INFO" | grep -o '"height":[0-9]*' | cut -d: -f2)
    NETWORK=$(echo "$INFO" | grep -o '"network":"[^"]*"' | cut -d'"' -f4)
    
    echo "  - Network: $NETWORK"
    echo "  - Current Height: $HEIGHT"
else
    echo -e "${RED}✗ Node is not accessible${NC}"
    echo
    echo "To start a testnet node:"
    echo "  cd deployments/testnet"
    echo "  docker-compose up -d"
fi

echo
echo "=== Test Complete ==="

# Create a simple HTML test page (output to current directory since web folder moved)
cat > test-results.html << 'EOF'
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Supernova Testnet Interface Test</title>
    <style>
        body {
            font-family: Arial, sans-serif;
            max-width: 800px;
            margin: 0 auto;
            padding: 20px;
            background: #f5f5f5;
        }
        .test-result {
            background: white;
            padding: 15px;
            margin: 10px 0;
            border-radius: 5px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .success { border-left: 4px solid #4CAF50; }
        .warning { border-left: 4px solid #FF9800; }
        .error { border-left: 4px solid #F44336; }
        h1 { color: #333; }
        .status { font-weight: bold; }
    </style>
</head>
<body>
    <h1>Supernova Testnet Interface Test Results</h1>
    <div id="results"></div>
    
    <script src="shared/api-utils.js"></script>
    <script>
        async function runTests() {
            const results = document.getElementById('results');
            
            // Test API endpoints
            const endpoints = [
                { path: '/blockchain/info', name: 'Blockchain Info' },
                { path: '/blockchain/stats', name: 'Blockchain Stats' },
                { path: '/mempool/info', name: 'Mempool Info' },
                { path: '/network/info', name: 'Network Info' },
                { path: '/node/info', name: 'Node Info' },
                { path: '/wallet/info', name: 'Wallet Info' },
                { path: '/faucet/status', name: 'Faucet Status' }
            ];
            
            for (const endpoint of endpoints) {
                const div = document.createElement('div');
                div.className = 'test-result';
                
                try {
                    const start = Date.now();
                    const data = await apiCall(endpoint.path);
                    const time = Date.now() - start;
                    
                    div.className += ' success';
                    div.innerHTML = `
                        <div class="status">✓ ${endpoint.name}</div>
                        <div>Response time: ${time}ms</div>
                        <div>Status: Connected</div>
                    `;
                } catch (error) {
                    div.className += ' error';
                    div.innerHTML = `
                        <div class="status">✗ ${endpoint.name}</div>
                        <div>Error: ${error.message}</div>
                    `;
                }
                
                results.appendChild(div);
            }
            
            // Test connection monitor
            const monitor = new ConnectionMonitor();
            const connected = await monitor.checkConnection();
            
            const monitorDiv = document.createElement('div');
            monitorDiv.className = 'test-result ' + (connected ? 'success' : 'error');
            monitorDiv.innerHTML = `
                <div class="status">Connection Monitor</div>
                <div>Status: ${connected ? 'Connected' : 'Disconnected'}</div>
            `;
            results.appendChild(monitorDiv);
        }
        
        // Run tests on page load
        runTests();
    </script>
</body>
</html>
EOF

echo
echo "Test results page created at: test-results.html"
echo "NOTE: Web interfaces have been moved to https://github.com/Carbon-Twelve-C12/supernova-web"
echo "Open this file in a browser to see interactive test results." 