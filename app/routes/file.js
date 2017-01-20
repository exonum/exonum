var express = require('express');
var request = require('request');
var router = express.Router();

var baseUrl = 'http://ts.exonum.com/f/';
var backendsUrl = 'http://exonum.com/backends/timestamping/content';

// router.post('/pay', function(req, res) {
//     var db = req.db;
//     var hash = req.body.label;
//     var description = req.body.description;
//
//     db.serialize(function() {
//         db.get('SELECT 1 FROM pairs WHERE hash = "' + hash + '"', function(err, row) {
//             if (typeof row === 'undefined') {
//                 db.prepare('INSERT INTO pairs (hash, description) VALUES (?, ?)').run(hash, description).finalize();
//             } else {
//                 db.run('UPDATE pairs SET description = "' + description + '" WHERE hash = "' + hash + '"');
//             }
//         });
//     });
//
//     res.redirect(307, 'https://money.yandex.ru/quickpay/confirm.xml');
// });

router.post('/upload', function(req, res) {
    var db = req.db;
    var hash = req.body.label;
    var description = req.body.description;

    db.serialize(function() {
        db.get('SELECT 1 FROM pairs WHERE hash = "' + hash + '"', function(err, row) {
            if (typeof row === 'undefined') {
                db.prepare('INSERT INTO pairs (hash, description) VALUES (?, ?)').run(hash, description).finalize(function() {
                    res.redirect(baseUrl + hash);
                });
            } else {
                db.run('UPDATE pairs SET description = "' + description + '" WHERE hash = "' + hash + '"', function(err) {
                    if (err !== null) {
                        res.render('error', {error: error});
                    } else {
                        res.redirect(baseUrl + hash);
                    }
                });
            }
        });
    });
});

// router.post('/proceed', function(req, res) {
//     var db = req.db;
//     var hash = req.body.label;
//
//     db.serialize(function() {
//         db.each('SELECT 1 rowid, * FROM pairs WHERE hash = "' + hash + '" LIMIT 1', function(err, row) {
//             if (typeof row !== 'undefined') {
//                 request.post({
//                     url: backendsUrl,
//                     headers: [{
//                         name: 'content-type',
//                         value: 'multipart/form-data'
//                     }],
//                     formData: {
//                         hash: hash,
//                         description: row.description
//                     }
//                 });
//             }
//         });
//     });
//
//     res.status(200).send();
// });

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
                } else if (response.statusCode === 409) {
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
