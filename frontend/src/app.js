import Vue from 'vue'
import App from './App.vue'
import router from './router'
import notify from './plugins/notify'
import moment from './plugins/moment'

Vue.use(notify)
Vue.use(moment)

new Vue({
  el: '#app',
  router,
  render: createElement => createElement(App)
})
