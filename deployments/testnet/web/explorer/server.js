const express = require('express');
const path = require('path');
const axios = require('axios');

const app = express();
const PORT = process.env.PORT || 3001;
const API_URL = process.env.API_URL || 'http://localhost:8332';

// Middleware
app.use(express.json());
app.use(express.static(path.join(__dirname, 'public')));

// CORS headers
app.use((req, res, next) => {
  res.header('Access-Control-Allow-Origin', '*');
  res.header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.header('Access-Control-Allow-Headers', 'Content-Type');
  next();
});

// Health check endpoint
app.get('/health', (req, res) => {
  res.json({ status: 'ok', service: 'block-explorer' });
});

// Proxy API calls to the blockchain node
app.get('/api/*', async (req, res) => {
  try {
    const apiPath = req.path.replace('/api', '');
    const response = await axios.get(`${API_URL}${apiPath}`, {
      params: req.query,
      timeout: 5000
    });
    res.json(response.data);
  } catch (error) {
    console.error('API proxy error:', error.message);
    res.status(error.response?.status || 500).json({
      error: error.message,
      status: error.response?.status || 500
    });
  }
});

// Serve the explorer HTML
app.get('/', (req, res) => {
  res.sendFile(path.join(__dirname, 'public', 'index.html'));
});

// Start server
app.listen(PORT, () => {
  console.log(`Block Explorer running on port ${PORT}`);
  console.log(`Proxying API calls to ${API_URL}`);
}); 