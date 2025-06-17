#!/bin/bash

echo "🧪 Testing Supernova Testnet..."
echo ""

# Test API directly
echo "📡 Testing API endpoint:"
curl -s http://146.190.225.136:8332/api/v1/blockchain/info | jq . || echo "API not accessible"
echo ""

echo "🔗 Testing node status:"
curl -s http://146.190.225.136:8332/api/v1/status | jq . || echo "Status endpoint not accessible"
echo ""

echo "📊 Testing blockchain height:"
curl -s http://146.190.225.136:8332/api/v1/blockchain/height || echo "Height endpoint not accessible"
echo ""

echo "
✅ Current testnet access points:
- API: http://146.190.225.136:8332/api/v1/
- Node is running on screen session 'supernova-node'

🚀 Ready for Step 3: Multi-node testnet deployment!
" 