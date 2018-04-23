import CryptoJS from 'crypto-js'

export default {
  install(Vue) {
    Vue.prototype.$crypto = {
      getHash: file => {
        return new Promise((resolve, reject) => {
          const reader = new FileReader;
          reader.onload = function() {
            try {
              const hash = CryptoJS.algo.SHA256.create()
              hash.update(CryptoJS.enc.Latin1.parse(reader.result))
              resolve('' + hash.finalize())
            } catch (error) {
              reject(error)
            }
          }

          reader.readAsBinaryString(file);
        })
      }
    }
  }
}
