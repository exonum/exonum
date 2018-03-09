const Vue = require('vue');
const router = require('./router');
const Storage = require('./plugins/storage');
const Validate = require('./plugins/validate');
const Notify = require('./plugins/notify');
const axios = require('./plugins/axios');
const App = require('./App.vue');

Vue.use(Storage);
Vue.use(Validate);
Vue.use(Notify);
Vue.use(axios);

Vue.mixin({
    data: function() {
        return {
            NETWORK_ID: 0,
            PROTOCOL_VERSION: 0,
            SERVICE_ID: 128,
            TX_WALLET_ID: 130,
            TX_ISSUE_ID: 129,
            TX_TRANSFER_ID: 128
        }
    }
});

new Vue({
    el: '#app',
    router,
    render: (createElement) => createElement(App)
});
