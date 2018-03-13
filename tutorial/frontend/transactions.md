# List of Transactions

Create list of transactions `src/pages/AuthPage.vue`:

```html
<template>
  <div>
    <div class="container">
      <div class="row">
        <div class="col-sm-12">

          <div class="card mt-5">...</div>

          <div class="card mt-5">
            <div class="card-header">Transactions</div>
            <ul class="list-group list-group-flush">

              <li class="list-group-item font-weight-bold">
                <div class="row">
                  <div class="col-sm-4">Hash</div>
                  <div class="col-sm-5">Description</div>
                  <div class="col-sm-3">Status</div>
                </div>
              </li>

              <li v-for="transaction in transactions" class="list-group-item">
                <div class="row">

                  <div class="col-sm-4"><code>{{ transaction.hash }}</code></div>

                  <div v-if="transaction.message_id == 130" class="col-sm-5">Wallet created</div>
                  <div v-else-if="transaction.message_id == 129" class="col-sm-5">
                    <strong v-numeral="transaction.body.amount"/> funds added
                  </div>
                  <div v-else-if="transaction.message_id == 128 && transaction.body.from == publicKey" class="col-sm-5">
                    <strong v-numeral="transaction.body.amount"/> sent to <code>{{ transaction.body.to }}</code>
                  </div>
                  <div v-else-if="transaction.message_id == 128 && transaction.body.to == publicKey" class="col-sm-5">
                    <strong v-numeral="transaction.body.amount"/> received from <code>{{ transaction.body.from }}</code>
                  </div>

                  <div class="col-sm-3">
                    <span v-if="transaction.status" class="badge badge-success">executed</span>
                    <span v-else class="badge badge-danger">failed</span>
                  </div>

                </div>
              </li>

            </ul>
          </div>

        </div>
      </div>
    </div>

    <modal :visible="isAddFundsModalVisible" title="Add Funds" action-btn="Add funds" @close="closeAddFundsModal" @submit="addFunds">...</modal>

    <modal :visible="isTransferModalVisible" title="Transfer Funds" action-btn="Transfer" @close="closeTransferModal" @submit="transfer">...</modal>

  </div>
</template>

<script>
  const Modal = require('../components/Modal.vue')

  module.exports = {
    components: {
      Modal
    },
    data: function() {...},
    methods: {...},
    mounted: function() {
      this.$nextTick(function() {
        const self = this

        this.$storage.get().then(function(keyPair) {
          self.$blockchain.getWallet(keyPair).then(function(data) {
            self.name = data.wallet.name
            self.publicKey = keyPair.publicKey
            self.balance = data.wallet.balance
            self.height = data.block.height
            self.transactions = data.transactions
          })
        })
      })
    }
  }
</script>
```

Modify `getWallet` method at `src/plugins/blockchain.js` plugin:

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
const TX_ISSUE_ID = 129
const TX_TRANSFER_ID = 128

const TableKey = Exonum.newType({...})
const Wallet = Exonum.newType({...})
const TransactionMetaData = Exonum.newType({
  size: 33,
  fields: {
    tx_hash: {type: Exonum.Hash, size: 32, from: 0, to: 32},
    execution_status: {type: Exonum.Bool, size: 1, from: 32, to: 33}
  }
})

function getTransaction(transactionId) {
  switch (transactionId) {
    case TX_WALLET_ID:
      return Exonum.newMessage({
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
    case TX_ISSUE_ID:
      return Exonum.newMessage({
        size: 48,
        network_id: NETWORK_ID,
        protocol_version: PROTOCOL_VERSION,
        service_id: SERVICE_ID,
        message_id: TX_ISSUE_ID,
        fields: {
          wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
          amount: {type: Exonum.Uint64, size: 8, from: 32, to: 40},
          seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
        }
      })
    case TX_TRANSFER_ID:
      return Exonum.newMessage({
        size: 80,
        network_id: NETWORK_ID,
        protocol_version: PROTOCOL_VERSION,
        service_id: SERVICE_ID,
        message_id: TX_TRANSFER_ID,
        fields: {
          from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
          to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
          amount: {type: Exonum.Uint64, size: 8, from: 64, to: 72},
          seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
        }
      })
    default:
      throw new Error('Unknown transaction ID has been passed')
  }
}

function getPublicKeyOfTransaction(transactionId, transaction) {
  switch (transactionId) {
    case TX_TRANSFER_ID:
      return transaction.from
    case TX_ISSUE_ID:
      return transaction.wallet
    case TX_WALLET_ID:
      return transaction.pub_key
    default:
      throw new Error('Unknown transaction ID has been passed')
  }
}

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

            // get transactions
            const transactionsMetaData = Exonum.merkleProof(
              wallet.history_hash,
              wallet.history_len,
              data.wallet_history.mt_proof,
              [0, wallet.history_len],
              TransactionMetaData
            )

            if (data.wallet_history.values.length !== transactionsMetaData.length) {
              // number of transactions in wallet history is not equal
              // to number of transactions in array with transactions meta data
              throw new Error('Transactions can not be verified')
            }

            // validate each transaction
            let transactions = []
            for (let i = 0; i < data.wallet_history.values.length; i++) {
              let Transaction = getTransaction(data.wallet_history.values[i].message_id)
              const publicKey = getPublicKeyOfTransaction(data.wallet_history.values[i].message_id, data.wallet_history.values[i].body)

              Transaction.signature = data.wallet_history.values[i].signature

              if (Transaction.hash(data.wallet_history.values[i].body) !== transactionsMetaData[i].tx_hash) {
                throw new Error('Invalid transaction hash has been found')
              } else if (!Transaction.verifySignature(data.wallet_history.values[i].signature, publicKey, data.wallet_history.values[i].body)) {
                throw new Error('Invalid transaction signature has been found')
              }

              transactions.push(Object.assign({
                hash: transactionsMetaData[i].tx_hash,
                status: transactionsMetaData[i].execution_status
              }, data.wallet_history.values[i]))
            }

            return {
              block: data.block_info.block,
              wallet: wallet,
              transactions: transactions
            }
          })
        })
      },

      addFunds: (keyPair, amountToAdd) => {...},

      transfer: (keyPair, receiver, amountToTransfer) => {...}

    }
  }
}
```
