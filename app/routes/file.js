var express = require('express');
var request = require('request');
var router = express.Router();

var baseUrl = 'http://127.0.0.1:2268/f/';
var backendsUrl = 'http://127.0.0.1:16000/timestamping/content';

router.post('/upload', function(req, res) {
    var hash = req.body.label;
    var description = req.body.description;

    request.post({
        url: backendsUrl,
        headers: [{
            name: 'content-type',
            value: 'multipart/form-data'
        }],
        formData: {
            hash: hash,
            description: description
        }
    }, function() {
        res.redirect(baseUrl + hash + '/redirect');
    });
});

router.get('/:hash/exists', function(req, res, next) {
    var hash = req.params.hash;

    request.get(backendsUrl + '/' + hash, function(error, response, body) {
        if (!error) {
            if (response.statusCode === 200) {
                res.json({exists: true, redirect: '/f/' + hash});
            } else if (body.type === 'FileNotFound') {
                res.json({exists: false});
            } else {
                res.render('error', {error: error});
            }
        } else {
            res.render('error', {error: error});
        }
    });
});

router.get('/:hash/redirect', function(req, res) {
    var hash = req.params.hash;
    var limit = 0;

    // start pooling until it will be able to get files info with GET request which means file is in a block
    (function pooling() {
        request.get({
            url: req.protocol + '://' + req.headers.host + '/f/' + hash + '/exists',
            json: true
        }, function(error, response, body) {
            if (!error) {
                if (body.exists === true) {
                    res.redirect(body.redirect);
                } else {
                    if (limit > 10) {
                        res.render('error');
                        return;
                    }
                    limit++;
                    setTimeout(function() {
                        pooling(res, hash);
                    }, 512);
                }
            } else {
                res.status(response.statusCode).send(error);
            }
        })
    })();
});

router.get('/:hash', function(req, res, next) {
    var hash = req.params.hash;

    request.get(backendsUrl + '/' + hash, function(error, response, body) {
        if (!error) {
            try {
                var data = JSON.parse(body);

                if (response.statusCode === 200) {
                    data['title'] = 'Certificate of proof ' + hash;
                    data['url'] = encodeURIComponent(baseUrl + hash);
                    res.render('file', data);
                } else if (data.type === 'FileNotFound') {
                    res.render('file-not-found', {title: 'File not found', hash: hash});
                } else {
                    res.render('error', {error: error});
                }
            } catch(e) {
                res.render('error');
            }
        } else {
            res.render('error', {error: error});
        }
    });
});

module.exports = router;
