const numeral = require('numeral');

module.exports = {
    install: function(Vue) {
        Vue.directive('numeral', {
            bind: function(el, binding) {
                el.innerHTML = numeral(binding.value).format('$0,0');
            },
            update: function(el, binding) {
                el.innerHTML = numeral(binding.value).format('$0,0');
            }
        })
    }
};
