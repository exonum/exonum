module.exports = function(config) {
    config.set({
        browsers: ['PhantomJS'],
        frameworks: ['mocha', 'chai', 'sinon'],
        files: [
            {pattern: 'node_modules/jquery/dist/jquery.min.js', watched: false},
            {pattern: 'node_modules/exonum-client/dist/exonum-client.min.js', watched: false},
            {pattern: 'node_modules/pwbox/dist/pwbox-lite.min.js', watched: false},
            {pattern: 'node_modules/big-integer/BigInteger.min.js', watched: false},
            {pattern: 'node_modules/karma-read-json/karma-read-json.js', watched: false},
            {pattern: 'node_modules/phantomjs-polyfill-object-assign/object-assign-polyfill.js', watched: false},
            {pattern: 'node_modules/phantomjs-polyfill-array-from/array-from-polyfill.js', watched: false},
            {pattern: 'js/cryptocurrency.js', watched: false},
            {pattern: 'test_data/**/*.json', included: false},
            'test/cryptocurrency.js'
        ],
        reporters: ['mocha'],
        singleRun: true
    })
};
