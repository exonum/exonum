import numeral from 'numeral'

export default {
  install(Vue) {
    Vue.directive('numeral', {
      bind(el, binding) {
        el.innerHTML = numeral(binding.value).format('$0,0')
      },
      update(el, binding) {
        el.innerHTML = numeral(binding.value).format('$0,0')
      }
    })
  }
}
