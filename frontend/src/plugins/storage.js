export default {
  install(Vue) {
    Vue.prototype.$storage = {
      set: function(keyPair) {
        localStorage.setItem('user', JSON.stringify(keyPair))
      },
      get: function() {
        return new Promise(function(resolve, reject) {
          let keyPair = JSON.parse(localStorage.getItem('user'))

          if (keyPair === null) {
            return reject(new Error('User not found in local storage'))
          }

          resolve(keyPair)
        })
      },
      remove: function() {
        localStorage.removeItem('user')
      }
    }
  }
}
