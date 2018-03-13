# Transfer Funds

Create modal with add funds form `src/pages/AuthPage.vue`:

```html
<template>
  <div>

    <div class="container">...</div>

    <modal :visible="isAddFundsModalVisible" title="Add Funds" action-btn="Add funds" @close="closeAddFundsModal" @submit="addFunds">...</modal>

    <modal :visible="isTransferModalVisible" title="Transfer Funds" action-btn="Transfer" @close="closeTransferModal" @submit="transfer">
      <div class="form-group">
        <label>Receiver:</label>
        <input v-model="receiver" type="text" class="form-control" placeholder="Enter public key">
      </div>
      <div class="form-group">
        <label>Amount:</label>
        <div class="input-group">
          <div class="input-group-prepend">
            <div class="input-group-text">$</div>
          </div>
          <input v-model="amountToTransfer" type="number" class="form-control" placeholder="Enter amount" min="1">
        </div>
      </div>
    </modal>

  <div>
</template>

<script>
  module.exports = {
    data: function() {
      return {
        name: '',
        publicKey: '',
        balance: 0,
        height: 0,
        amountToAdd: 10,
        receiver: '',
        amountToTransfer: '',
        isAddFundsModalVisible: false,
        isTransferModalVisible: false,
        variants: [
          {id: 'ten', amount: 10},
          {id: 'fifty', amount: 50},
          {id: 'hundred', amount: 100}
        ]
      }
    },
    methods: {

      openAddFundsModal: function() {...},

      closeAddFundsModal: function() {...},

      addFunds: function() {...},

      openTransferModal: function() {
        this.isTransferModalVisible = true
      },

      closeTransferModal: function() {
        this.isTransferModalVisible = false
      },

      transfer: function() {
        const self = this

        this.$storage.get().then(function(keyPair) {
          self.$blockchain.transfer(keyPair, self.receiver, self.amountToTransfer).then(function() {
            self.isTransferModalVisible = false
          })
        }).catch(function(error) {
          self.isTransferModalVisible = false
          throw error
        })
      }

    },
    mounted: function() {...}
  }
</script>
```

Define `transfer` method at `src/plugins/blockchain.js` plugin:

```javascript
...

module.exports = {
  install: function(Vue) {
    Vue.prototype.$blockchain = {

      createWallet: name => {...},

      getWallet: keyPair => {...},

      addFunds: (keyPair, amountToAdd) => {...},

      transfer: (keyPair, receiver, amountToTransfer) => {
        const TxTransfer = Exonum.newMessage({
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

        const data = {
          from: keyPair.publicKey,
          to: receiver,
          amount: amountToTransfer,
          seed: Exonum.randomUint64()
        }

        const signature = TxTransfer.sign(keyPair.secretKey, data)

        return axios.post(TX_URL, {
          network_id: NETWORK_ID,
          protocol_version: PROTOCOL_VERSION,
          service_id: SERVICE_ID,
          message_id: TX_TRANSFER_ID,
          signature: signature,
          body: data
        })
      }

    }
  }
}
```

Next step is to create [list of transactions](transactions.md).
