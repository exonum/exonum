const express = require('express')
const { createProxyMiddleware } = require('http-proxy-middleware')

// Initialize application
const app = express()

// Get app params
const argv = require('yargs-parser')(process.argv.slice(2))
const port = argv.port
const apiRoot = argv.apiRoot

if (typeof port === 'undefined') {
  throw new Error('--port parameter is not set.');
}

if (typeof apiRoot === 'undefined') {
  throw new Error('--api-root parameter is not set.');
}

// Set path to static files
app.use(express.static(__dirname + '/'));

// Proxy middleware options
const apiProxy = createProxyMiddleware({
  target: apiRoot,
  ws: true,
  headers: {
    'Origin': 'http://localhost'
  }
});

app.use('/api', apiProxy);

app.listen(port);
