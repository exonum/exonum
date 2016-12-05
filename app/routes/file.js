var express = require('express');
var request = require('request');
var router = express.Router();

var backendsUrl = 'http://exonum.com/backends/timestamping/content';

router.post('/proceed', function(req, res) {
    var hash = req.body.label;
    var description = '';

    // create file
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
    });

    res.status(200).send();
});

router.get('/:hash/exists', function(req, res, next) {
    var hash = req.params.hash;

    request.get(backendsUrl + '/' + hash, function(error, response, body) {
        if (!error) {
            if (response.statusCode === 200) {
                res.json({exists: true, redirect: '/f/' + hash});
            } else if (response.statusCode === 409) {
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

    // start pooling until it will be able to get files info with GET request which means file is in a block
    (function pooling() {
        request.get({
            url: req.protocol + '://' + req.headers.host + '/f/' + hash + '/exists',
            json: true
        }, function(error, response, body) {
            if (!error) {
                if (body.exists) {
                    res.redirect(body.redirect);
                } else {
                    setTimeout(function() {
                        pooling(res, hash);
                    }, 128);
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
            var data = JSON.parse(body);

            if (response.statusCode === 200) {
                data['title'] = 'Certificate of proof';
                data['url'] = encodeURIComponent('http://ts.exonum.com/f/' + hash);

                res.render('file', data);
            } else if (response.statusCode === 409) {
                res.render('file-not-found', {title: 'File not found', hash: hash});
            } else {
                res.render('error', {error: error});
            }
        } else {
            res.render('error', {error: error});
        }
    });
});

module.exports = router;
