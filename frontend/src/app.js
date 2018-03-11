const Vue = require('vue');
const router = require('./router');
const numeral = require('./directives/numeral');
const Storage = require('./plugins/storage');
const Validate = require('./plugins/validate');
const Notify = require('./plugins/notify');
const Blockchain = require('./plugins/blockchain');
const App = require('./App.vue');

Vue.use(numeral);
Vue.use(Storage);
Vue.use(Validate);
Vue.use(Notify);
Vue.use(Blockchain);

new Vue({
    el: '#app',
    router,
    render: (createElement) => createElement(App)
});
