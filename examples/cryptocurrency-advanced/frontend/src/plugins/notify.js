import Noty from 'noty'

export default {
  install(Vue) {
    Vue.prototype.$notify = function(type = 'information', text) {
      new Noty({
        theme: 'bootstrap-v4',
        timeout: 5000,
        type: type,
        text: text,
        killer: true,
        progressBar: false
      }).show()
    }
  }
}
