# Registration Form

Create template for authorization page `src/pages/AuthPage.vue`:

```html
<template>
  <div class="container">
    <div class="row justify-content-sm-center">
      <div class="col-md-6 col-md-offset-3">

        <h1 class="mt-5 mb-4">Authorization</h1>

        <form @submit.prevent="register">
          <div class="form-group">
            <label class="control-label">Name:</label>
            <input v-model="name" type="text" class="form-control" placeholder="Enter name" maxlength="260">
          </div>
          <button type="submit" class="btn btn-lg btn-block btn-primary">Register</button>
        </form>

      </div>
    </div>
  </div>
</template>

<script>
  module.exports = {
    data: function() {
      return {
        keyPair: {}
      }
    },
    methods: {

      register: function() {
        const self = this

        this.$blockchain.createWallet(this.name).then(function(keyPair) {
          self.name = ''
          self.keyPair = keyPair
        })
      }

    }
  }
</script>
```

Create plugin with global-level blockchain functionality
`src/plugins/blockchain.js` and define `createWallet` method:

```javascript
import * as Exonum from 'exonum-client'
import axios from 'axios'

const TX_URL = '/api/services/cryptocurrency/v1/wallets/transaction'

const NETWORK_ID = 0
const PROTOCOL_VERSION = 0
const SERVICE_ID = 128
const TX_WALLET_ID = 130

module.exports = {
  install: function(Vue) {
    Vue.prototype.$blockchain = {

      createWallet: name => {
        const keyPair = Exonum.keyPair()

        const TxCreateWallet = Exonum.newMessage({
          size: 40,
          network_id: NETWORK_ID,
          protocol_version: PROTOCOL_VERSION,
          service_id: SERVICE_ID,
          message_id: TX_WALLET_ID,
          fields: {
            pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            name: {type: Exonum.String, size: 8, from: 32, to: 40}
          }
        })

        const data = {
          pub_key: keyPair.publicKey,
          name: name
        }

        const signature = TxCreateWallet.sign(keyPair.secretKey, data)

        return axios.post(TX_URL, {
          network_id: NETWORK_ID,
          protocol_version: PROTOCOL_VERSION,
          service_id: SERVICE_ID,
          message_id: TX_WALLET_ID,
          signature: signature,
          body: data
        }).then(() => {
          return keyPair
        })
      }

    }
  }
}
```

Create modal with data singing key pair of newly created user `src/pages/AuthPage.vue`:

```html
<template>
  <div>

    <div class="container">...</div>

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

  </div>
</template>
<script>
  const Modal = require('../components/Modal.vue')

  module.exports = {
    components: {
      Modal
    },
    data: function() {
      return {
        isModalVisible: false,
        keyPair: {}
      }
    },
    methods: {

      register: function() {...},

      closeModal: function() {
        this.isModalVisible = false
      },

      proceed: function() {
        this.isModalVisible = false

        this.$storage.set(this.keyPair)

        this.$router.push({name: 'user'})
      }

    }
  }
</script>
```

Check sources of [modal component](frontend/src/components/Modal.vue).

Key pair of authorized used is stored in browser's localStorage.
Check sources of [storage plugin](frontend/src/plugins/storage.js).
Don't forget to include `storage` plugin into `app.js`:

```javascript
import Storage from './plugins/storage'

Vue.use(Storage)
```

Next step is to create [login form](login-form.md).
