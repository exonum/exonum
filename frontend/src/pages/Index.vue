<template>
  <div>
    <div class="container mt-5">
      <div class="row justify-content-sm-center">
        <div class="col-md-6 col-md-offset-3">
          <h1>Timestamp the file</h1>

          <form class="mt-5" @submit.prevent="timestamp">
            <div class="form-group">
              <label class="control-label">File:</label>
              <input v-model="hash" type="text" class="form-control" placeholder="Enter hash" required>
            </div>
            <div class="form-group">
              <label class="control-label">Metadata:</label>
              <input v-model="metadata" type="text" class="form-control" placeholder="Enter metadata">
              <small class="form-text text-muted">Optional field.</small>
            </div>
            <button type="submit" class="btn btn-lg btn-block btn-primary">Make a timestamp</button>
          </form>
        </div>
      </div>
    </div>

    <spinner :visible="isSpinnerVisible"/>
  </div>
</template>

<script>
  import Spinner from '../components/Spinner.vue'

  module.exports = {
    components: {
      Spinner
    },
    data() {
      return {
        isSpinnerVisible: false
      }
    },
    methods: {
      async timestamp() {
        this.isSpinnerVisible = true

        try {
          await this.$blockchain.createTimestamp(this.hash, this.metadata)
          this.isSpinnerVisible = false
          this.$nextTick(function() {
            this.$router.push({ name: 'timestamp', params: { hash: this.hash } })
          })
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
      }
    }
  }
</script>
