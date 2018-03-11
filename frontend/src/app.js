import Vue from 'vue'
import router from './router'
import numeral from './directives/numeral'
import Storage from './plugins/storage'
import Validate from './plugins/validate'
import Notify from './plugins/notify'
import Blockchain from './plugins/blockchain'
import App from './App.vue'

Vue.use(numeral)
Vue.use(Storage)
Vue.use(Validate)
Vue.use(Notify)
Vue.use(Blockchain)

new Vue({
  el: '#app',
  router,
  render: (createElement) => createElement(App)
})
