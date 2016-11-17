// # Custom equation helper
// Usage: `{{#is_multiple_of_three index}} ... {{/is_multiple_of_three}}`

var is_multiple_of_three = function(value, options) {
    options = options || {};

    if (value > 0 && value % 3 === 0) {
        return options.fn(this);
    }
    return options.inverse(this);
};

module.exports = is_multiple_of_three;
