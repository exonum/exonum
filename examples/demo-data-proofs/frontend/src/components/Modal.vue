<template>
  <div>
    <div :class="{ 'show d-block': visible }" class="modal" tabindex="-1" role="dialog">
      <div class="modal-dialog" role="document">
        <div class="modal-content">
          <form @submit.prevent="submit">
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
              <button type="submit" class="btn btn-primary">{{ actionBtn }}</button>
            </div>
          </form>
        </div>
      </div>
    </div>

    <div v-if="visible" class="modal-backdrop"/>
  </div>
</template>

<script>
  const className = 'modal-open'

  module.exports = {
    name: 'modal',
    props: {
      title: String,
      actionBtn: String,
      visible: Boolean
    },
    watch: {
      visible(value) {
        this.toggle(value)
      }
    },
    methods: {
      close() {
        this.$emit('close')
      },

      submit() {
        this.$emit('submit')
      },

      toggle(state) {
        if (state) {
          document.body.classList.add(className)
        } else {
          document.body.classList.remove(className)
        }
      }
    },
    mounted() {
      this.$nextTick(function() {
        this.toggle(false)
      })
    }
  }
</script>

<style>
  .modal-backdrop {
    opacity: 0.25;
  }
</style>
