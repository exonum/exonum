<template>
  <div>
    <navbar/>

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
              <!-- eslint-disable-next-line vue/require-v-for-key -->
              <li v-for="(transaction) in transactions" class="list-group-item">
                <div class="row">
                  <div class="col-sm-12">
                    <router-link :to="{ name: 'transaction', params: { hash: transaction } }">{{ transaction }}</router-link>
                  </div>
                </div>
              </li>
              <li v-if="transactions.length === 0" class="list-group-item">
                <div class="row">
                  <div class="col-sm-12">
                    <em class="text-secondary">There are no transactions in the block</em>
                  </div>
                </div>
              </li>
            </ul>
          </div>

          <nav class="mt-5" aria-label="Nearby blocks navigation">
            <ul class="pagination justify-content-center">
              <li class="page-item">
                <router-link :to="{ name: 'block', params: { height: previous } }" class="page-link">&larr; Previous block</router-link>
              </li>
              <li class="page-item">
                <router-link :to="{ name: 'block', params: { height: next } }" class="page-link">Next block &rarr;</router-link>
              </li>
            </ul>
          </nav>
        </div>
      </div>
    </div>

    <spinner :visible="isSpinnerVisible"/>
  </div>
</template>

<script>
  const Navbar = require('../components/Navbar.vue')
  const Spinner = require('../components/Spinner.vue')

  module.exports = {
    components: {
      Navbar,
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
    watch: {
      height: function() {
        this.loadBlock()
      }
    },
    methods: {
      loadBlock: function() {
        const self = this

        this.isSpinnerVisible = true

        this.$blockchain.getBlock(this.height).then(data => {
          self.isSpinnerVisible = false
          self.block = data.block
          self.transactions = data.txs
        }).catch(error => {
          self.$notify('error', error.toString())
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
