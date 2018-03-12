<template>
  <div>
    <div :class="{ 'show d-block': visible }" class="modal" tabindex="-1" role="dialog">
      <div class="modal-dialog" role="document">
        <div class="modal-content">
          <div class="modal-header">
            <h5 class="modal-title">{{ title }}</h5>
            <button type="button" class="close" @click="close">
              <span aria-hidden="true">&times;</span>
            </button>
          </div>

          <div class="modal-body">
            <slot/>
          </div>

          <div class="modal-footer">
            <button type="button" class="btn btn-secondary" @click="close">Close</button>
            <button type="button" class="btn btn-primary" @click="action">{{ actionBtn }}</button>
          </div>
        </div>
      </div>
    </div>

    <div v-if="visible" class="modal-backdrop"/>
  </div>
</template>

<script>
  module.exports = {
    name: 'modal',
    props: {
      title: String,
      actionBtn: String,
      visible: Boolean
    },
    watch: {
      visible: function(state) {
        const className = 'modal-open'

        if (state) {
          document.body.classList.add(className)
        } else {
          document.body.classList.remove(className)
        }
      }
    },
    methods: {
      close: function() {
        this.$emit('close')
      },
      action: function() {
        this.$emit('submit')
      }
    }
  }
</script>

<style>
  .modal-backdrop {
    opacity: 0.25;
  }
</style>
