# Add Funds

Create modal with add funds form `src/pages/AuthPage.vue`:

```html
<template>
  <div>

    <div class="container">...</div>

    <modal :visible="isAddFundsModalVisible" title="Add Funds" action-btn="Add funds" @close="closeAddFundsModal" @submit="addFunds">
      <div class="form-group">
        <label class="d-block">Select amount to be added:</label>
        <div v-for="variant in variants" class="form-check form-check-inline">
          <input :id="variant.id" :value="variant.amount" :checked="amountToAdd == variant.amount" v-model="amountToAdd" class="form-check-input" type="radio">
          <label :for="variant.id" class="form-check-label">${{ variant.amount }}</label>
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
        isAddFundsModalVisible: false,
        variants: [
          {id: 'ten', amount: 10},
          {id: 'fifty', amount: 50},
          {id: 'hundred', amount: 100}
        ]
      }
    },
    methods: {

      openAddFundsModal: function() {
        this.isAddFundsModalVisible = true
      },

      closeAddFundsModal: function() {
        this.isAddFundsModalVisible = false
      },

      addFunds: function() {
        const self = this

        this.$storage.get().then(function(keyPair) {
          self.$blockchain.addFunds(keyPair, self.amountToAdd).then(function() {
            self.isAddFundsModalVisible = false
          })
        }).catch(function(error) {
          self.isAddFundsModalVisible = false
          throw error
        })
      }

    },
    mounted: function() {...}
  }
</script>
```

Define `addFunds` method at `src/plugins/blockchain.js` plugin:

```javascript
...

module.exports = {
  install: function(Vue) {
    Vue.prototype.$blockchain = {

      createWallet: name => {...},

      getWallet: keyPair => {...},

      addFunds: (keyPair, amountToAdd) => {
        const TxIssue = Exonum.newMessage({
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

        const data = {
          wallet: keyPair.publicKey,
          amount: amountToAdd.toString(),
          seed: Exonum.randomUint64()
        }

        const signature = TxIssue.sign(keyPair.secretKey, data)

        return axios.post(TX_URL, {
          network_id: NETWORK_ID,
          protocol_version: PROTOCOL_VERSION,
          service_id: SERVICE_ID,
          message_id: TX_ISSUE_ID,
          signature: signature,
          body: data
        })
      }

    }
  }
}
```

Next step is to create [transfer funds](transfer-funds.md) interface.
