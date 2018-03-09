const axios = require('axios');

module.exports = {
    install: function(Vue) {
        Object.defineProperty(Vue.prototype, '$http', {value: axios});
    }
};
