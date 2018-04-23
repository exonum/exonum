// This router is just a simple proxy between application and node's public API

var express = require('express');
var request = require('request');
var router = express.Router();

router.get('/*', function(req, res, next) {
  var query = req.params[0];

  request.get({
    url: req.app.get('apiRoot') + '/api/' + query,
    qs: req.query
  }, function(err, response, body) {
    if (err) {
      return next(err);
    }
    try {
      res.json(JSON.parse(body));
    } catch (e) {
      res.json({});
    }
  });
});

router.post('/*', function(req, res, next) {
  var query = req.params[0];

  request.post({
      url: req.app.get('apiRoot') + '/api/' + query,
      json: req.body
    },
    function(err, response, body) {
      if (err) {
        return next(err);
      }
      res.json(body);
    });
});

module.exports = router;
