var express = require('express');
var request = require('request');
var formidable = require('formidable');
var fs = require('fs');
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

router.post('/create', function(req, res, next) {
    var form = new formidable.IncomingForm();
    form.uploadDir = 'uploads/';
    form.keepExtensions = true;

    // keep original file name
    form.on('file', function(field, file) {
        fs.rename(file.path, form.uploadDir + "/" + file.name);
        file.path = form.uploadDir + "/" + file.name;
    });

    form.parse(req, function(err, fields, files) {
        if (err) {
            res.status(500).send('Unknown error');
            return false;
        }

        request.post({
            url: 'http://exonum.com/backends/timestamping/content',
            headers: [{
                name: 'content-type',
                value: 'multipart/form-data'
            }],
            formData: {
                description: fields.description,
                content: {
                    value: fs.createReadStream(files.content.path),
                    options: {
                        filename: files.content.name,
                        contentType: files.content.type
                    }
                }
            }
        }, function(error, response, body) {
            if (!error) {
                if (response.statusCode === 200) {
                    var data = JSON.parse(body);

                    // start pooling until it will be able to get files info with GET request which means file is in a block
                    pooling(res, data.hash);
                } else if (response.statusCode === 409) { // file exists
                    res.send({redirect: '/f/exists'});
                } else {
                    res.status(response.statusCode).send(error);
                }
            } else {
                res.status(response.statusCode).send(error);
            }

            // remove local file
            fs.unlink(files.content.path);
        });
    });
});

function pooling(res, hash) {
    request.get('http://exonum.com/backends/timestamping/info/' + hash, function(error, response, body) {
        if (!error) {
            if (response.statusCode === 200) {
                res.send({redirect: '/f/' + hash + '/success'});
            } else if (response.statusCode === 409) { // file not found
                setTimeout(function() {
                    pooling(res, hash);
                }, 100);
            } else {
                res.status(response.statusCode).send(error);
            }
        } else {
            res.status(response.statusCode).send(error);
        }
    })
}

module.exports = router;
