# Faucet Service Dockerfile
FROM node:20-alpine

# Install dependencies
RUN apk add --no-cache curl

# Create app directory
WORKDIR /app

# Create non-root user
RUN adduser -D -u 1000 faucet

# Copy static files
COPY index.html styles.css script.js ./public/

# Create simple Express server for the faucet
RUN cat > server.js << 'EOF'
const express = require('express');
const path = require('path');
const app = express();

const PORT = process.env.PORT || 3002;
const NODE_URL = process.env.NODE_URL || 'http://localhost:8332';
const FAUCET_AMOUNT = parseFloat(process.env.FAUCET_AMOUNT || '10');
const RATE_LIMIT_MINUTES = parseInt(process.env.RATE_LIMIT_MINUTES || '60');

// Rate limiting storage
const requestHistory = new Map();

app.use(express.json());
app.use(express.static('public'));

// CORS headers
app.use((req, res, next) => {
  res.header('Access-Control-Allow-Origin', '*');
  res.header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.header('Access-Control-Allow-Headers', 'Content-Type');
  next();
});

// Health check endpoint
app.get('/health', (req, res) => {
  res.json({ status: 'ok' });
});

// Faucet API endpoint
app.post('/api/faucet', (req, res) => {
  const { address } = req.body;
  const clientIp = req.ip;
  
  // Check rate limit
  const now = Date.now();
  const lastRequest = requestHistory.get(clientIp);
  
  if (lastRequest && (now - lastRequest) < RATE_LIMIT_MINUTES * 60 * 1000) {
    const waitTime = Math.ceil((RATE_LIMIT_MINUTES * 60 * 1000 - (now - lastRequest)) / 1000 / 60);
    return res.status(429).json({ 
      error: `Rate limit exceeded. Please wait ${waitTime} minutes.` 
    });
  }
  
  // Update request history
  requestHistory.set(clientIp, now);
  
  // Clean old entries
  for (const [ip, time] of requestHistory.entries()) {
    if (now - time > RATE_LIMIT_MINUTES * 60 * 1000 * 2) {
      requestHistory.delete(ip);
    }
  }
  
  // Mock response for now
  res.json({
    success: true,
    txid: '0x' + Math.random().toString(16).substr(2, 64),
    amount: FAUCET_AMOUNT,
    message: `${FAUCET_AMOUNT} NOVA sent to ${address}`
  });
});

app.listen(PORT, () => {
  console.log(`Faucet service running on port ${PORT}`);
});
EOF

# Create package.json
RUN cat > package.json << 'EOF'
{
  "name": "supernova-faucet",
  "version": "1.0.0",
  "description": "Supernova Testnet Faucet",
  "main": "server.js",
  "scripts": {
    "start": "node server.js"
  },
  "dependencies": {
    "express": "^4.18.2"
  }
}
EOF

# Install dependencies
RUN npm install --production

# Change ownership
RUN chown -R faucet:faucet /app

USER faucet

# Faucet port
EXPOSE 3002

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:3002/health || exit 1

# Default command
CMD ["npm", "start"] 