<template>
  <div>
    <div v-if="timestamp" class="container">
      <div class="row justify-content-sm-center">
        <div class="col-md-6 col-md-offset-3">
          <h1 class="mt-5 mb-4">File is stamped</h1>
          <ul class="list-group mt-3">
            <li class="list-group-item">
              <div class="row">
                <div class="col-sm-3"><strong>Content hash:</strong></div>
                <div class="col-sm-9">{{ timestamp.content_hash }}</div>
              </div>
            </li>
            <li class="list-group-item">
              <div class="row">
                <div class="col-sm-3"><strong>Metadata:</strong></div>
                <div class="col-sm-9">{{ timestamp.metadata }}</div>
              </div>
            </li>
            <li class="list-group-item">
              <div class="row">
                <div class="col-sm-3"><strong>Transaction:</strong></div>
                <div class="col-sm-9">
                  <code>
                    <router-link :to="{ name: 'transaction', params: { hash: transactionHash } }">{{ transactionHash }}</router-link>
                  </code>
                </div>
              </div>
            </li>
            <li class="list-group-item">
              <div class="row">
                <div class="col-sm-3"><strong>Time:</strong></div>
                <div class="col-sm-9">{{ $moment(time) }}</div>
              </div>
            </li>
          </ul>
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
      async loadTimestamp() {
        this.isSpinnerVisible = true

        try {
          const data = await this.$blockchain.getTimestampProof(this.hash)
          if (data === 'Content hash already exists') {
            this.isSpinnerVisible = false
            this.$notify('error', error.toString())
            this.$nextTick(function() {
              this.$router.push({ name: 'index' })
            })
            return
          }
          this.timestamp = data.timestamp
          this.transactionHash = data.tx_hash
          this.time = data.time
          this.isSpinnerVisible = false
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
      }
    },
    mounted: function() {
      this.$nextTick(function() {
        this.loadTimestamp()
      })
    }
  }
</script>
