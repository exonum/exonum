import numeral from 'numeral'

export default {
  install: function(Vue) {
    Vue.directive('numeral', {
      bind: function(el, binding) {
        el.innerHTML = numeral(binding.value).format('$0,0')
      },
      update: function(el, binding) {
        el.innerHTML = numeral(binding.value).format('$0,0')
      }
    })
  }
}
