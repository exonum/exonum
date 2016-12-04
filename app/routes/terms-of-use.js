var express = require('express');
var router = express.Router();

router.get('/', function(req, res, next) {
    res.render('terms', {title: 'Terms of Use'});
});

module.exports = router;
