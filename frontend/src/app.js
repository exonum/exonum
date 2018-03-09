const Vue = require('vue');
const router = require('./router');
const App = require('./App.vue');

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
