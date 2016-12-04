var express = require('express');
var path = require('path');
var favicon = require('serve-favicon');
var logger = require('morgan');
var cookieParser = require('cookie-parser');
var bodyParser = require('body-parser');
var hbs = require('hbs');
var moment = require('moment');

var index = require('./routes/index');
var privacy = require('./routes/privacy-policy');
var terms = require('./routes/terms-of-use');
var faq = require('./routes/faq');
var verify = require('./routes/verify');
var file = require('./routes/file');
var create = require('./routes/create');

var app = express();

// view engine setup
app.set('views', path.join(__dirname, 'views'));
app.set('view engine', 'hbs');

hbs.registerPartials(__dirname + '/views/partials');
hbs.registerHelper('formatDate', function(date, format) {
    var mmnt = moment(date, 'X');
    return mmnt.format(format);
});

app.use(favicon(path.join(__dirname, 'public', 'favicon.ico')));
app.use(logger('dev'));
app.use(bodyParser.json());
app.use(bodyParser.urlencoded({extended: false}));
app.use(cookieParser());
app.use(express.static(path.join(__dirname, 'public')));

app.use('/', index);
app.use('/privacy-policy', privacy);
app.use('/terms-of-use', terms);
app.use('/faq', faq);
app.use('/verify', verify);
app.use('/f', file);
app.use('/create', create);

// catch 404 and forward to error handler
app.use(function(req, res, next) {
    var err = new Error('Not Found');
    err.status = 404;
    next(err);
});

// error handler
app.use(function(err, req, res, next) {
    // set locals, only providing error in development
    res.locals.message = err.message;
    res.locals.error = req.app.get('env') === 'development' ? err : {};

    // render the error page
    res.status(err.status || 500);
    res.render('error');
});

module.exports = app;
