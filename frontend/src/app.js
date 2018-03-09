const Vue = require('vue');
const router = require('./router');
const App = require('./App.vue');

new Vue({
    el: '#app',
    router,
    render: (createElement) => createElement(App)
});
