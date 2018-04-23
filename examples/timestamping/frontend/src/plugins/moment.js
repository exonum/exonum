import moment from 'moment'
import bigInt from 'big-integer'

export default {
  install(Vue) {
    Vue.prototype.$moment = systemTime => {
      const timestamp = bigInt(systemTime.secs).multiply(1000).plus(bigInt(systemTime.nanos).over(1000000)).valueOf()
      return moment(timestamp).format('DD.MM.YYYY, HH:mm:ss')
    }
  }
}
