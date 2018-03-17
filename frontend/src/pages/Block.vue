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
              <li class="breadcrumb-item active" aria-current="page">Block {{ height }}</li>
            </ol>
          </nav>

          <div class="card mt-5">
            <div class="card-header">Transactions</div>
            <ul class="list-group list-group-flush">
              <li class="list-group-item font-weight-bold">
                <div class="row">
                  <div class="col-sm-12">Hash</div>
                </div>
              </li>
              <li v-for="(transaction) in transactions" class="list-group-item">
                <div class="row">
                  <div class="col-sm-12">
                    <router-link :to="{ name: 'transaction', params: { hash: transaction } }">{{ transaction }}</router-link>
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
  const Spinner = require('../components/Spinner.vue')

  module.exports = {
    components: {
      Spinner
    },
    props: {
      height: String
    },
    data: function() {
      return {
        block: Object,
        transactions: Array
      }
    },
    computed: {
      previous: function() {
        return (parseInt(this.height) - 1).toString()
      },
      next: function() {
        return (parseInt(this.height) + 1).toString()
      }
    },
    methods: {
      loadBlock: function() {
        const self = this

        this.isSpinnerVisible = true

        this.$http.get('/api/explorer/v1/blocks/' + this.height).then(response => {
          self.isSpinnerVisible = false

          if (typeof response.data === 'object') {
            self.block = response.data.block
            self.transactions = response.data.txs
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
        this.loadBlock()
      })
    }
  }
</script>
