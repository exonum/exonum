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
                  <div class="col-sm-9"><code>{{ keyPair.publicKey }}</code></div>
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
                  <div class="col-sm-12">Description</div>
                </div>
              </li>
              <!-- eslint-disable-next-line vue/require-v-for-key -->
              <li v-for="transaction in reverseTransactions" class="list-group-item">
                <div class="row">
                  <div class="col-sm-12">
                    <router-link :to="{ name: 'transaction', params: { hash: transaction.hash } }">
                      <span v-if="transaction.name">Wallet created</span>
                      <span v-else-if="transaction.to && transaction.to === keyPair.publicKey">
                        <strong v-numeral="transaction.amount"/> funds received
                      </span>
                      <span v-else-if="transaction.to">
                        <strong v-numeral="transaction.amount"/> funds sent
                      </span>
                      <span v-else>
                        <strong v-numeral="transaction.amount"/> funds added
                      </span>
                    </router-link>
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
                  <div v-for="variant in variants" :key="variant.id" class="form-check form-check-inline">
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
  import { mapState } from 'vuex'
  import Modal from '../components/Modal.vue'
  import Navbar from '../components/Navbar.vue'
  import Spinner from '../components/Spinner.vue'

  module.exports = {
    components: {
      Modal,
      Navbar,
      Spinner
    },
    data() {
      return {
        name: '',
        balance: 0,
        amountToAdd: 10,
        receiver: '',
        amountToTransfer: '',
        isSpinnerVisible: false,
        transactions: [],
        variants: [
          { id: 'ten', amount: 10 },
          { id: 'fifty', amount: 50 },
          { id: 'hundred', amount: 100 }
        ]
      }
    },
    computed: Object.assign({
      reverseTransactions() {
        return this.transactions.slice().reverse()
      }
    }, mapState({
      keyPair: state => state.keyPair
    })),
    methods: {
      async loadUser() {
        if (this.keyPair === null) {
          this.$store.commit('logout')
          this.$router.push({ name: 'home' })
          return
        }

        this.isSpinnerVisible = true

        try {
          const data = await this.$blockchain.getWallet(this.keyPair.publicKey)
          this.name = data.wallet.name
          this.balance = data.wallet.balance
          this.transactions = data.transactions
          this.isSpinnerVisible = false
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
      },

      async addFunds() {
        this.isSpinnerVisible = true

        const seed = this.$blockchain.generateSeed()

        try {
          await this.$blockchain.addFunds(this.keyPair, this.amountToAdd, seed)
          const data = await this.$blockchain.getWallet(this.keyPair.publicKey)
          this.balance = data.wallet.balance
          this.transactions = data.transactions
          this.isSpinnerVisible = false
          this.$notify('success', 'Add funds transaction has been written into the blockchain')
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
      },

      async transfer() {
        if (!this.$validateHex(this.receiver)) {
          return this.$notify('error', 'Invalid public key is passed')
        }

        if (this.receiver === this.keyPair.publicKey) {
          return this.$notify('error', 'Can not transfer funds to yourself')
        }

        this.isSpinnerVisible = true

        const seed = this.$blockchain.generateSeed()

        try {
          await this.$blockchain.transfer(this.keyPair, this.receiver, this.amountToTransfer, seed)
          const data = await this.$blockchain.getWallet(this.keyPair.publicKey)
          this.balance = data.wallet.balance
          this.transactions = data.transactions
          this.isSpinnerVisible = false
          this.$notify('success', 'Transfer transaction has been written into the blockchain')
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
      }
    },
    mounted() {
      this.$nextTick(function() {
        this.loadUser()
      })
    }
  }
</script>
