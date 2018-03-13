# Login Form

Modify template of authorization page and add login form
`src/pages/AuthPage.vue`:

```html
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
              <form @submit.prevent="register">...</form>
            </tab>

          </tabs>

        </div>
      </div>
    </div>

    <modal :visible="isModalVisible" title="Wallet has been created" action-btn="Log in" @close="closeModal" @submit="proceed">...<modal>
  </div>
</template>
<script>
  const Tab = require('../components/Tab.vue')
  const Tabs = require('../components/Tabs.vue')
  const Modal = require('../components/Modal.vue')

  module.exports = {
    components: {
      Tab,
      Tabs,
      Modal
    },
    data: function() {...},
    methods: {

      login: function() {
        this.$storage.set({
          publicKey: this.publicKey,
          secretKey: this.secretKey
        })

        this.$router.push({name: 'user'})
      },

      register: function() {...},

      closeModal: function() {...},

      proceed: function() {...}

    }
  }
</script>
```

Check sources of tab component components at
[src/components/Tabs.vue](frontend/src/components/Tabs.vue) and
[src/components/Tab.vue](frontend/src/components/Tab.vue).

Next step is to create [user cabinet](user-cabinet.md).
