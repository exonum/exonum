var express = require('express');
var app = express();
var path = require('path');

var commandLineArgs = require('command-line-args');
var optionDefinitions = [{name: 'config', type: String}];
var options = commandLineArgs(optionDefinitions);
var configPath = options.config || './config.json';
var config = require(configPath);
app.set('config', config);

var bodyParser = require('body-parser');
app.use(bodyParser.json());
app.use(bodyParser.urlencoded({extended: true}));

app.use(express.static(__dirname + '/'));

var api = require('./routes/api');
var configuration = require('./routes/configuration');
app.use('/api', api);
app.use('/configuration', configuration);

app.get('/', function(req, res) {
    res.sendFile('index.html');
});

app.listen(3000);