var express = require('express');
var router = express.Router();

router.get('/', function(req, res, next) {
    var config = req.app.get('config');
    res.json({
        network_id: config.network_id,
        protocol_version: config.protocol_version,
        service_id: config.service_id,
        validators: config.validators
    });
});

module.exports = router;
