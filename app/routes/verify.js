var express = require('express');
var router = express.Router();

router.get('/', function(req, res, next) {
    res.render('verify', {title: 'Verify a stamp'});
});

module.exports = router;
