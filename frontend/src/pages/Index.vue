<template>
  <div>
    <div class="container">
      <div class="row justify-content-sm-center">
        <div class="col-md-6 col-md-offset-3">
          <h1 class="mt-5 mb-4">Timestamp file</h1>
          <form @submit.prevent="timestamp">
            <div class="form-group">
              <label class="control-label">Hash:</label>
              <input v-model="hash" type="text" class="form-control" placeholder="Enter name" maxlength="260" required>
            </div>
            <button type="submit" class="btn btn-lg btn-block btn-primary">Timestamp</button>
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
          const keyPair = await this.$blockchain.createTimestamp(this.name)
          this.isSpinnerVisible = false
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
      }
    }
  }
</script>
