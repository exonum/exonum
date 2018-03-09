const Vue = require('vue');
const Router = require('vue-router');
const AuthPage = require('../pages/AuthPage.vue');
const WalletPage = require('../pages/WalletPage.vue');

Vue.use(Router);

module.exports = new Router({
    routes: [
        {
            path: '/',
            name: 'home',
            component: AuthPage
        },
        {
            path: '/user',
            name: 'user',
            component: WalletPage
        }
    ]
});
