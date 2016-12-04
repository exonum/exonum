var express = require('express');
var router = express.Router();

router.get('/', function(req, res, next) {
    res.render('faq', {title: 'Frequently Asked Questions'});
});

module.exports = router;
