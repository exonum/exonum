import Vue from 'vue'
import Router from 'vue-router'
import IndexPage from '../pages/Index.vue'
import TimestampPage from '../pages/Timestamp.vue'

Vue.use(Router)

export default new Router({
  routes: [
    {
      path: '/',
      name: 'index',
      component: IndexPage
    },
    {
      path: '/timestamp/:hash',
      name: 'hash',
      component: TimestampPage,
      props: true
    }
  ]
})
