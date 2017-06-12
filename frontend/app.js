var express = require('express');
var app = express();
var path = require('path');
var bodyParser = require('body-parser');
var api = require('./routes/api');
var configuration = require('./routes/configuration');

app.use(bodyParser.json());
app.use(bodyParser.urlencoded({extended: true}));

app.use(express.static(__dirname + '/'));

app.use('/api', api);
app.use('/configuration', configuration);

app.get('/', function(req, res) {
    res.sendFile('index.html');
});

app.listen(3000);