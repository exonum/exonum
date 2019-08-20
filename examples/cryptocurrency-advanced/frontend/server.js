var express = require('express');
var proxy = require('http-proxy-middleware');

// Initialize application
var app = express();

// Get app params
var argv = require('yargs-parser')(process.argv.slice(2));
var port = argv.port;
var apiRoot = argv.apiRoot;

if (typeof port === 'undefined') {
  throw new Error('--port parameter is not set.');
}

if (typeof apiRoot === 'undefined') {
  throw new Error('--api-root parameter is not set.');
}

// Set path to static files
app.use(express.static(__dirname + '/'));

// Proxy middleware options
var apiProxy = proxy({
  target: apiRoot,
  ws: true,
  headers: {
    'Origin': 'http://localhost'
  }
});

app.use('/api', apiProxy);

app.listen(port);
