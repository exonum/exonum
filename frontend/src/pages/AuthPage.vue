<template>
  <div>
    <div class="container">
      <div class="row justify-content-sm-center">
        <div class="col-md-6 col-md-offset-3">
          <h1 class="mt-5 mb-4">Authorization</h1>
          <tabs>
            <tab :is-active="true" title="Log in">
              <form @submit.prevent="login">
                <div class="form-group">
                  <label class="control-label">Public key:</label>
                  <input v-model="publicKey" type="text" class="form-control" placeholder="Enter public key">
                </div>
                <div class="form-group">
                  <label class="control-label">Secret key:</label>
                  <input v-model="secretKey" type="text" class="form-control" placeholder="Enter secret key">
                </div>
                <button type="submit" class="btn btn-lg btn-block btn-primary">Log in</button>
              </form>
            </tab>
            <tab title="Register">
              <form @submit.prevent="register">
                <div class="form-group">
                  <label class="control-label">Name:</label>
                  <input v-model="name" type="text" class="form-control" placeholder="Enter name" maxlength="260">
                </div>
                <button type="submit" class="btn btn-lg btn-block btn-primary">Register</button>
              </form>
            </tab>
          </tabs>
        </div>
      </div>
    </div>

    <modal :visible="isModalVisible" title="Wallet has been created" action-btn="Log in" @close="closeModal" @submit="proceed">
      <div class="alert alert-warning" role="alert">Save the key pair in a safe place. You will need it to log in to the demo next time.</div>
      <div class="form-group">
        <label>Public key:</label>
        <div><code>{{ keyPair.publicKey }}</code></div>
      </div>
      <div class="form-group">
        <label>Secret key:</label>
        <div><code>{{ keyPair.secretKey }}</code></div>
      </div>
    </modal>

    <spinner :visible="isSpinnerVisible"/>
  </div>
</template>

<script>
  const Tab = require('../components/Tab.vue')
  const Tabs = require('../components/Tabs.vue')
  const Modal = require('../components/Modal.vue')
  const Spinner = require('../components/Spinner.vue')

  module.exports = {
    components: {
      Tab,
      Tabs,
      Modal,
      Spinner
    },
    data: function() {
      return {
        isModalVisible: false,
        isSpinnerVisible: false,
        keyPair: {}
      }
    },
    methods: {
      login: function() {
        if (!this.$validateHex(this.publicKey)) {
          return this.$notify('error', 'Invalid public key is passed')
        }

        if (!this.$validateHex(this.secretKey, 64)) {
          return this.$notify('error', 'Invalid secret key is passed')
        }

        this.isSpinnerVisible = true

        this.$storage.set({
          publicKey: this.publicKey,
          secretKey: this.secretKey
        })

        this.$router.push({name: 'user'})
      },

      register: function() {
        const self = this

        if (!this.name) {
          return this.$notify('error', 'The name is a required field')
        }

        this.isSpinnerVisible = true

        this.$blockchain.createWallet(this.name).then(function(keyPair) {
          self.name = ''
          self.keyPair = keyPair
          self.isSpinnerVisible = false
          self.isModalVisible = true
        }).catch(function(error) {
          self.isSpinnerVisible = false
          self.$notify('error', error.toString())
        })
      },

      closeModal: function() {
        this.isModalVisible = false
      },

      proceed: function() {
        this.isModalVisible = false

        this.$storage.set(this.keyPair)

        this.$nextTick(function() {
          this.$router.push({name: 'user'})
        })
      }
    }
  }
</script>
