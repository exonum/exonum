var express = require('express');
var request = require('request');
var formidable = require('formidable');
var fs = require('fs');
var router = express.Router();

router.post('/', function(req, res, next) {
    var form = new formidable.IncomingForm();
    form.uploadDir = 'uploads/';
    form.keepExtensions = true;

    // TODO do not modify original file

    form.parse(req, function(err, fields, files) {
        if (err) {
            res.render('error');
            return false;
        }

        request.post({
            url: 'http://exonum.com/backends/timestamping/content',
            headers: [
                {
                    name: 'content-type',
                    value: 'multipart/form-data'
                }
            ],
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
                    res.send({redirect: '/f/' + data.hash + '/success'});
                } else if (response.statusCode === 400) {
                    // TODO redirect to 'file exists' page
                } else {
                    res.render('error');
                }
            } else {
                res.render('error');
            }

            // TODO remove local file
        });
    });
});

module.exports = router;
