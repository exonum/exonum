<template>
  <div>
    <navbar/>

    <div class="container">
      <div class="row">
        <div class="col-md-6">
          <div class="card mt-5">
            <div class="card-header">User summary</div>
            <ul class="list-group list-group-flush">
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Name:</strong></div>
                  <div class="col-sm-9">{{ name }}</div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Public key:</strong></div>
                  <div class="col-sm-9"><code>{{ publicKey }}</code></div>
                </div>
              </li>
              <li class="list-group-item">
                <div class="row">
                  <div class="col-sm-3"><strong>Balance:</strong></div>
                  <div class="col-sm-9">
                    <span v-numeral="balance"/>
                  </div>
                </div>
              </li>
            </ul>
          </div>

          <div class="card mt-5">
            <div class="card-header">Transactions</div>
            <ul class="list-group list-group-flush">
              <li class="list-group-item font-weight-bold">
                <div class="row">
                  <div class="col-sm-8">Description</div>
                  <div class="col-sm-4">Status</div>
                </div>
              </li>
              <!-- eslint-disable-next-line vue/require-v-for-key -->
              <li v-for="transaction in reverseTransactions" class="list-group-item">
                <div class="row">
                  <div class="col-sm-8">
                    <router-link :to="{ name: 'transaction', params: { hash: transaction.hash } }">
                      <span v-if="transaction.message_id == 130">You wallet created</span>
                      <span v-else-if="transaction.message_id == 129">
                        <strong v-numeral="transaction.body.amount"/> funds added
                      </span>
                      <span v-else-if="transaction.message_id == 128 && transaction.body.from == publicKey">
                        <strong v-numeral="transaction.body.amount"/> sent
                      </span>
                      <span v-else-if="transaction.message_id == 128 && transaction.body.to == publicKey">
                        <strong v-numeral="transaction.body.amount"/> received
                      </span>
                    </router-link>
                  </div>
                  <div class="col-sm-4">
                    <span v-if="transaction.status" class="badge badge-success">Accepted</span>
                    <span v-else class="badge badge-danger">Rejected</span>
                  </div>
                </div>
              </li>
            </ul>
          </div>
        </div>
        <div class="col-md-6">
          <div class="card mt-5">
            <div class="card-header">Add funds</div>
            <div class="card-body">
              <form @submit.prevent="addFunds">
                <div class="form-group">
                  <label class="d-block">Select amount to be added:</label>
                  <!-- eslint-disable-next-line vue/require-v-for-key -->
                  <div v-for="variant in variants" class="form-check form-check-inline">
                    <input :id="variant.id" :value="variant.amount" :checked="amountToAdd == variant.amount" v-model="amountToAdd" class="form-check-input" type="radio">
                    <label :for="variant.id" class="form-check-label">${{ variant.amount }}</label>
                  </div>
                </div>
                <button type="submit" class="btn btn-primary">Add funds</button>
              </form>
            </div>
          </div>

          <div class="card mt-5">
            <div class="card-header">Transfer funds</div>
            <div class="card-body">
              <form @submit.prevent="transfer">
                <div class="form-group">
                  <label>Receiver:</label>
                  <input v-model="receiver" type="text" class="form-control" placeholder="Enter public key" required>
                </div>
                <div class="form-group">
                  <label>Amount:</label>
                  <div class="input-group">
                    <div class="input-group-prepend">
                      <div class="input-group-text">$</div>
                    </div>
                    <input v-model="amountToTransfer" type="number" class="form-control" placeholder="Enter amount" min="0" required>
                  </div>
                </div>
                <button type="submit" class="btn btn-primary">Transfer funds</button>
              </form>
            </div>
          </div>
        </div>
      </div>
    </div>

    <spinner :visible="isSpinnerVisible"/>
  </div>
</template>

<script>
  const Modal = require('../components/Modal.vue')
  const Navbar = require('../components/Navbar.vue')
  const Spinner = require('../components/Spinner.vue')

  module.exports = {
    components: {
      Modal,
      Navbar,
      Spinner
    },
    data: function() {
      return {
        name: '',
        publicKey: '',
        balance: 0,
        amountToAdd: 10,
        receiver: '',
        amountToTransfer: '',
        isSpinnerVisible: false,
        transactions: [],
        variants: [
          {id: 'ten', amount: 10},
          {id: 'fifty', amount: 50},
          {id: 'hundred', amount: 100}
        ]
      }
    },
    computed: {
      reverseTransactions: function() {
        return this.transactions.slice().reverse()
      }
    },
    methods: {
      loadUser: function() {
        const self = this

        if (this.$store.state.keyPair === null) {
          this.$store.commit('logout')
          this.$router.push({name: 'home'})
          return
        }

        this.isSpinnerVisible = true

        this.$blockchain.getWallet(this.$store.state.keyPair).then(data => {
          self.name = data.wallet.name
          self.publicKey = this.$store.state.keyPair.publicKey
          self.balance = data.wallet.balance
          self.transactions = data.transactions
          self.isSpinnerVisible = false
        }).catch(function(error) {
          self.isSpinnerVisible = false
          self.$notify('error', error.toString())
        })
      },

      addFunds: function() {
        const self = this

        this.isSpinnerVisible = true

        this.$blockchain.addFunds(this.$store.state.keyPair, this.amountToAdd).then(data => {
          self.balance = data.wallet.balance
          self.transactions = data.transactions
          self.isSpinnerVisible = false
          self.$notify('success', 'Add funds transaction has been written into the blockchain')
        }).catch(function(error) {
          self.isSpinnerVisible = false
          self.$notify('error', error.toString())
        })
      },

      transfer: function() {
        const self = this

        if (!this.$validateHex(this.receiver)) {
          return this.$notify('error', 'Invalid public key is passed')
        }

        if (this.receiver === this.$store.state.keyPair.publicKey) {
          return self.$notify('error', 'Can not transfer funds to yourself')
        }

        this.isSpinnerVisible = true

        this.$blockchain.transfer(this.$store.state.keyPair, this.receiver, this.amountToTransfer).then(data => {
          self.balance = data.wallet.balance
          self.transactions = data.transactions
          self.isSpinnerVisible = false
          self.$notify('success', 'Transfer transaction has been written into the blockchain')
        }).catch(function(error) {
          self.isSpinnerVisible = false
          self.$notify('error', error.toString())
        })
      }
    },
    mounted: function() {
      this.$nextTick(function() {
        this.loadUser()
      })
    }
  }
</script>
