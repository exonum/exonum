var express = require('express');
var router = express.Router();

var config = require('../config.json');

router.get('/', function(req, res, next) {
    res.json({
        network_id: config.network_id,
        protocol_version: config.protocol_version,
        service_id: config.service_id,
        validators: config.validators
    });
});

module.exports = router;
