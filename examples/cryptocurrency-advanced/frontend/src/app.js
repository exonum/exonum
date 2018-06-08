import 'babel-polyfill';
import Vue from 'vue'
import router from './router'
import numeral from './directives/numeral'
import Validate from './plugins/validate'
import Notify from './plugins/notify'
import Blockchain from './plugins/blockchain'
import App from './App.vue'
import store from './store'

Vue.use(numeral)
Vue.use(Validate)
Vue.use(Notify)
Vue.use(Blockchain)

new Vue({
  el: '#app',
  router,
  store,
  render: createElement => createElement(App)
})
