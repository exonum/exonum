<template>
  <div>
    <navbar/>

    <div class="container">
      <div class="row">
        <div class="card-header">Transfer funds</div>
        <div class="card-body">
          <form @submit.prevent="createDuel">
            <div class="form-group">
              <label>Игрок 1:</label>
              <input v-model="player1_key" type="text" class="form-control" placeholder="" required>
            </div>
            <div class="form-group">
              <label>Игрок 2:</label>
              <input v-model="player2_key" type="text" class="form-control" placeholder="" required>
            </div>
            <div class="form-group">
              <label>Судья 1:</label>
              <input v-model="judge1_key" type="text" class="form-control" placeholder="" required>
            </div>
            <div class="form-group">
              <label>Судья 2:</label>
              <input v-model="judge2_key" type="text" class="form-control" placeholder="" required>
            </div>
            <div class="form-group">
              <label>Судья 3:</label>
              <input v-model="judge3_key" type="text" class="form-control" placeholder="" required>
            </div>
            <div class="form-group">
              <label>Номер ситуации:</label>
              <input v-model="situation_number" type="number" class="form-control" placeholder="" required>
            </div>
            <button type="submit" class="btn btn-primary">Создать поединок</button>
          </form>          
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
  import * as Exonum from 'exonum-client'

  module.exports = {
    components: {
      Modal,
      Navbar,
      Spinner
    },
    data() {
      return {
        player1_key: Exonum.keyPair().publicKey,
        player2_key: Exonum.keyPair().publicKey,
        judge1_key: Exonum.keyPair().publicKey,
        judge2_key: Exonum.keyPair().publicKey,
        judge3_key: Exonum.keyPair().publicKey,
        situation_number: null,
        isSpinnerVisible: false,
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

        this.isSpinnerVisible = false
        
        try {
          /*
          const data = await this.$blockchain.getDuel(this.keyPair.publicKey)
          this.player1_key = data.duel.player1_key
          this.player2_key = data.duel.player2_key
          this.judge1_key = data.duel.judge1_key
          this.judge2_key = data.duel.judge2_key
          this.judge3_key = data.duel.judge3_key
          this.situation_number = data.duel.situation_number
          */
        } catch (error) {
          this.isSpinnerVisible = false
          this.$notify('error', error.toString())
        }
        
      },
      /*
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
      */
      async createDuel() {
        if (!this.$validateHex(this.keyPair.publicKey)) {
          return this.$notify('error', 'Invalid public key is passed')
        }
        
        if (this.situation_number === null) {
          return this.$notify('error', 'Empty situation_number')
        }
        
        this.isSpinnerVisible = true

        const seed = this.$blockchain.generateSeed()

        try {
          await this.$blockchain.createDuel(this.keyPair, this.player1_key, this.player2_key, this.judge1_key, this.judge2_key, this.judge3_key, this.situation_number)
          /*
          const data = await this.$blockchain.getDuel(this.keyPair.publicKey)
          this.player1_key = data.duel.player1_key
          this.player2_key = data.duel.player2_key
          this.judge1_key = data.duel.judge1_key
          this.judge2_key = data.duel.judge2_key
          this.judge3_key = data.duel.judge3_key
          this.situation_number = data.duel.situation_number
          */
          this.$notify('success', 'Transfer transaction has been written into the blockchain')
          this.isSpinnerVisible = false
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
