import Vue from 'vue'
import Router from 'vue-router'
import AuthPage from '../pages/AuthPage.vue'
import WalletPage from '../pages/WalletPage.vue'

Vue.use(Router)

export default new Router({
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
})
