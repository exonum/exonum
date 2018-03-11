const axios = require('axios');

module.exports = {
    install: function(Vue) {
        Vue.prototype.$http = axios;
    }
};
