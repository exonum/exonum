<template>
  <div>
    <div class="container">
      <div class="row">
        <div class="col-sm-12">
          <nav class="mt-5" aria-label="breadcrumb">
            <ol class="breadcrumb">
              <li class="breadcrumb-item">
                <router-link :to="{ name: 'blockchain' }">Blockchain</router-link>
              </li>
              <li class="breadcrumb-item">
                <router-link :to="{ name: 'block', params: { height: location.block_height } }">Block {{ location.block_height }}</router-link>
              </li>
              <li class="breadcrumb-item active" aria-current="page">Transaction {{ hash }}</li>
            </ol>
          </nav>

          <div class="card mt-5">
            <div class="card-header">Transaction</div>
            <ul class="list-group list-group-flush">
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
                  <div class="col-sm-9">{{ type }}</div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Protocol version:</strong></div>
                  <div class="col-sm-9">{{ transaction.protocol_version }}</div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Network ID:</strong></div>
                  <div class="col-sm-9">{{ transaction.network_id }}</div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Service ID:</strong></div>
                  <div class="col-sm-9">{{ transaction.service_id }}</div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Message ID:</strong></div>
                  <div class="col-sm-9">{{ transaction.message_id }}</div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Signature:</strong></div>
                  <div class="col-sm-9">{{ transaction.signature }}</div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Body:</strong></div>
                  <div class="col-sm-9"><pre><code>{{ JSON.stringify(transaction.body, null, 2) }}</code></pre></div>
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
  const Spinner = require('../components/Spinner.vue')

  module.exports = {
    components: {
      Spinner
    },
    props: {
      hash: String
    },
    data: function() {
      return {
        transaction: Object,
        location: Object,
        type: String
      }
    },
    methods: {
      loadTransaction: function() {
        const self = this

        this.$http.get('/api/system/v1/transactions/' + this.hash).then(response => {
          if (typeof response.data === 'object') {
            self.transaction = response.data.content
            self.location = response.data.location
            self.type = response.data.type
          } else {
            self.$notify('success', 'Unknown format of server response')
          }
        }).catch(error => {
          self.$notify('success', error.toString())
        })
      }
    },
    mounted: function() {
      this.$nextTick(function() {
        this.loadTransaction()
      })
    }
  }
</script>
