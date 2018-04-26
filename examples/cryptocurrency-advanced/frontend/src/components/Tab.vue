<template>
  <div :class="{ 'active': active }" class="tab-pane">
    <slot/>
  </div>
</template>

<script>
  module.exports = {
    name: 'tab',
    data() {
      return {
        active: false
      }
    },
    props: {
      title: String,
      isActive: {
        required: false,
        type: Boolean,
        default: false
      }
    },
    created() {
      this.active = this.isActive
    },
    mounted() {
      this.$parent.addTab(this)
    }
  }
</script>
