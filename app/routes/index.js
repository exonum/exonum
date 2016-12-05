var express = require('express');
var router = express.Router();

router.get('/', function(req, res, next) {
    res.render('index', {title: 'Create a stamp'});
});

router.get('/verify', function(req, res, next) {
    res.render('verify', {title: 'Verify a stamp'});
});

router.get('/privacy-policy', function(req, res, next) {
    res.render('privacy', {title: 'Privacy Policy'});
});

router.get('/terms-of-use', function(req, res, next) {
    res.render('terms', {title: 'Terms of Use'});
});

router.get('/faq', function(req, res, next) {
    res.render('faq', {title: 'Frequently Asked Questions'});
});

module.exports = router;
