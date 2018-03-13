# User Cabinet

Create template for user cabinet page `src/pages/WalletPage.vue`:

```html
<template>
  <div class="container">
    <div class="row">
      <div class="col-sm-12">

        <div class="card mt-5">
          <div class="card-header">User summary</div>
          <ul class="list-group list-group-flush">

            <li class="list-group-item">
              <div class="row">
                <div class="col-sm-3"><strong>Name:</strong></div>
                <div class="col-sm-9">
                  {{ name }}
                </div>
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

            <li class="list-group-item">
              <div class="row">
                <div class="col-sm-3"><strong>Block:</strong></div>
                <div class="col-sm-9">{{ height }}</div>
              </div>
            </li>

          </ul>
        </div>

      </div>
    </div>
  </div>
</template>

<script>
  module.exports = {
    data: function() {
      return {
        name: '',
        publicKey: '',
        balance: 0,
        height: 0
      }
    },
    mounted: function() {
      this.$nextTick(function() {
        const self = this

        this.$storage.get().then(function(keyPair) {
          self.$blockchain.getWallet(keyPair).then(function(data) {
            self.name = data.wallet.name
            self.publicKey = keyPair.publicKey
            self.balance = data.wallet.balance
            self.height = data.block.height
          })
        })
      })
    }
  }
</script>
```

Modify router `src/router/index.js`:

```javascript
import Vue from 'vue'
import Router from 'vue-router'
import AuthPage from '../pages/AuthPage.vue'
import WalletPage from '../pages/WalletPage.vue'

Vue.use(Router)

export default new Router({
  routes: [
    {
      path: '/',
      name: 'home',
      component: AuthPage
    },
    {
      path: '/user',
      name: 'user',
      component: WalletPage
    }
  ]
})
```

Define `getWallet` method at `src/plugins/blockchain.js` plugin:

```javascript
import * as Exonum from 'exonum-client'
import axios from 'axios'

const TX_URL = '/api/services/cryptocurrency/v1/wallets/transaction'
const CONFIG_URL = '/api/services/configuration/v1/configs/actual'
const WALLET_URL = '/api/services/cryptocurrency/v1/wallets/info?pubkey='

const NETWORK_ID = 0
const PROTOCOL_VERSION = 0
const SERVICE_ID = 128
const TX_WALLET_ID = 130

const TableKey = Exonum.newType({
  size: 4,
  fields: {
    service_id: {type: Exonum.Uint16, size: 2, from: 0, to: 2},
    table_index: {type: Exonum.Uint16, size: 2, from: 2, to: 4}
  }
})
const Wallet = Exonum.newType({
  size: 88,
  fields: {
    pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
    name: {type: Exonum.String, size: 8, from: 32, to: 40},
    balance: {type: Exonum.Uint64, size: 8, from: 40, to: 48},
    history_len: {type: Exonum.Uint64, size: 8, from: 48, to: 56},
    history_hash: {type: Exonum.Hash, size: 32, from: 56, to: 88}
  }
})

module.exports = {
  install: function(Vue) {
    Vue.prototype.$blockchain = {

      createWallet: name => {...},

      getWallet: keyPair => {
        return axios.get(CONFIG_URL).then(response => {
          // actual list of validators
          const validators = response.data.config.validator_keys.map(validator => {
            return validator.consensus_key
          })

          return axios.get(WALLET_URL + keyPair.publicKey).then(response => {
            return response.data
          }).then((data) => {
            if (!Exonum.verifyBlock(data.block_info, validators, NETWORK_ID)) {
              throw new Error('Block can not be verified')
            }

            // find root hash of table with wallets in the tree of all tables
            const tableKey = TableKey.hash({
              service_id: SERVICE_ID,
              table_index: 0
            })
            const walletsHash = Exonum.merklePatriciaProof(data.block_info.block.state_hash, data.wallet.mpt_proof, tableKey)
            if (walletsHash === null) {
              throw new Error('Wallets table not found')
            }

            // find wallet in the tree of all wallets
            const wallet = Exonum.merklePatriciaProof(walletsHash, data.wallet.value, keyPair.publicKey, Wallet)
            if (wallet === null) {
              throw new Error('Wallet not found')
            }

            return {
              block: data.block_info.block,
              wallet: wallet
            }
          })
        })
      }

    }
  }
}
```

Next step is to create [add funds](add-funds.md) interface.
