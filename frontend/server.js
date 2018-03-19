var express = require('express');
var bodyParser = require('body-parser');

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

app.set('apiRoot', apiRoot);

// Configure parsers
app.use(bodyParser.json());
app.use(bodyParser.urlencoded({extended: true}));

// Set path to static files
app.use(express.static(__dirname + '/'));

// Activate routers
var api = require('./routes/api');
app.use('/api', api);

// Single Page Application entry point
app.get('/', function(req, res) {
  res.sendFile('index.html');
});

app.listen(port);
