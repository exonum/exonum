<template>
  <div :class="{ 'active': active }" class="tab-pane">
    <slot />
  </div>
</template>

<script>
  export default {
    name: 'Tab',
    props: {
      title: {
        type: String,
        default: ""
      },
      isActive: {
        required: false,
        type: Boolean,
        default: false
      }
    },
    data() {
      return {
        active: false
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
