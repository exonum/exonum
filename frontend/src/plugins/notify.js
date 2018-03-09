const Noty = require('noty');

module.exports = {
    install: function(Vue) {
        /**
         * Show notification
         * @param type
         * @param text
         * @returns {boolean}
         */
        Vue.notify = function(type = 'information', text) {
            new Noty({
                theme: 'bootstrap-v4',
                timeout: 5000,
                type: type,
                text: text,
                killer: true,
                progressBar: false
            }).show();
        }
    }
};
