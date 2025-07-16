const http = require('http');

// Mock data for testnet
const mockData = {
  nodeInfo: {
    height: 12345,
    connections: 8,
    uptime: 86400
  },
  blockchainInfo: {
    height: 12345,
    best_block_hash: "00000000abcdef1234567890",
    difficulty: 1.5,
    total_work: "0x1234567890",
    network: "supernova-testnet",
    version: "1.0.0"
  },
  mempoolInfo: {
    transaction_count: 3
  },
  blockchainStats: {
    hashrate: 15000,
    total_transactions: 42000
  }
};

const server = http.createServer((req, res) => {
  res.setHeader('Content-Type', 'application/json');
  res.setHeader('Access-Control-Allow-Origin', '*');
  
  if (req.url === '/api/v1/node/info') {
    res.writeHead(200);
    res.end(JSON.stringify(mockData.nodeInfo));
  } else if (req.url === '/api/v1/blockchain/info') {
    res.writeHead(200);
    res.end(JSON.stringify(mockData.blockchainInfo));
  } else if (req.url === '/api/v1/mempool/info') {
    res.writeHead(200);
    res.end(JSON.stringify(mockData.mempoolInfo));
  } else if (req.url === '/api/v1/blockchain/stats') {
    res.writeHead(200);
    res.end(JSON.stringify(mockData.blockchainStats));
  } else {
    res.writeHead(404);
    res.end(JSON.stringify({ error: 'Not found' }));
  }
});

server.listen(8332, 'localhost', () => {
  console.log('Mock Supernova API running on http://localhost:8332');
}); 