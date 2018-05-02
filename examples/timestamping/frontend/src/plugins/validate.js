export default {
  install(Vue) {
    Vue.prototype.$validate = {
      hex: (hash, bytes) => {
        bytes = bytes || 32

        if (typeof hash !== 'string') {
          return false
        } else if (hash.length !== bytes * 2) {
          // hexadecimal string is of wrong length
          return false
        }

        for (let i = 0; i < hash.length; i++) {
          if (isNaN(parseInt(hash[i], 16))) {
            // invalid symbol in hexadecimal string
            return false
          }
        }

        return true
      }
    }
  }
}
