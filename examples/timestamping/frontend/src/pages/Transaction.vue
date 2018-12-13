<template>
  <div>
    <div class="container mt-5">
      <div class="row justify-content-sm-center">
        <div class="col-md-6 col-md-offset-3">
          <h1>Transaction</h1>

          <div v-if="location.block_height" class="card mt-5">
            <div class="card-header">Transaction</div>
            <ul class="list-group list-group-flush">
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Hash:</strong></div>
                  <div class="col-sm-9">
                    <code>{{ hash }}</code>
                  </div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Block:</strong></div>
                  <div class="col-sm-9">
                    <router-link :to="{ name: 'block', params: { height: location.block_height } }">{{ location.block_height }}</router-link>
                  </div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Type:</strong></div>
                  <div class="col-sm-9">
                    <code>{{ type }}</code>
                  </div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Status:</strong></div>
                  <div class="col-sm-9">
                    <code>{{ status.type }}</code>
                  </div>
                </div>
              </li>
              <li v-if="content.message" class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Serialized:</strong></div>
                  <div class="col-sm-9">
                    <code>{{ content.message }}</code>
                  </div>
                </div>
              </li>
              <li v-if="content.debug" class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Content:</strong></div>
                  <div class="col-sm-9">
                    <pre><code>{{ JSON.stringify(content.debug, null, 2) }}</code></pre>
                  </div>
                </div>
              </li>
            </ul>
          </div>
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
    props: ['hash'],
    data() {
      return {
        content: {},
        location: {},
        status: {},
        type: '',
        isSpinnerVisible: false
      }
    },
    methods: {
      async loadTransaction() {
        this.isSpinnerVisible = true

        try {
          const data = await this.$blockchain.getTransaction(this.hash)
          this.content = data.content
          this.location = data.location
          this.status = data.status
          this.type = data.type
          this.isSpinnerVisible = false
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
      }
    },
    mounted() {
      this.$nextTick(function() {
        this.loadTransaction()
      })
    }
  }
</script>
