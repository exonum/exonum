import Vue from 'vue'
import App from './App.vue'
import router from './router'
import blockchain from './plugins/blockchain'
import crypto from './plugins/crypto'
import moment from './plugins/moment'
import notify from './plugins/notify'
import validate from './plugins/validate'

Vue.use(blockchain)
Vue.use(crypto)
Vue.use(moment)
Vue.use(notify)
Vue.use(validate)

new Vue({
  el: '#app',
  router,
  render: createElement => createElement(App)
})
