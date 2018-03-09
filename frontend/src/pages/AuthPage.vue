<template>
    <div>
        <div class="container">
            <div class="row justify-content-sm-center">
                <div class="col-md-6 col-md-offset-3">
                    <h1 class="mt-5 mb-4">Authorization</h1>
                    <tabs>
                        <tab title="Log in" v-bind:isActive="true">
                            <form v-on:submit.prevent="login">
                                <div class="form-group">
                                    <label class="control-label">Public key:</label>
                                    <input type="text" class="form-control" placeholder="Enter public key" v-model="publicKey">
                                </div>
                                <div class="form-group">
                                    <label class="control-label">Secret key:</label>
                                    <input type="password" class="form-control" placeholder="Enter secret key" v-model="secretKey">
                                </div>
                                <button type="submit" class="btn btn-lg btn-block btn-primary">Log in</button>
                            </form>
                        </tab>
                        <tab title="Register">
                            <form v-on:submit.prevent="register">
                                <div class="form-group">
                                    <label class="control-label">Name:</label>
                                    <input type="text" class="form-control" placeholder="Enter name" maxlength="260" v-model="name">
                                </div>
                                <button type="submit" class="btn btn-lg btn-block btn-primary">Register</button>
                            </form>
                        </tab>
                    </tabs>
                </div>
            </div>
        </div>

        <modal title="Wallet has been created" actionBtn="Log in" v-bind:visible="isModalVisible" v-on:close="closeModal" v-on:submit="proceed">
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

        <spinner v-bind:visible="isSpinnerVisible"></spinner>
    </div>
</template>

<script>
    const Vue = require('vue');
    const Exonum = require('exonum-client');
    const Tab = require('../components/Tab.vue');
    const Tabs = require('../components/Tabs.vue');
    const Modal = require('../components/Modal.vue');
    const Spinner = require('../components/Spinner.vue');

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
                if (!Vue.validateHexString(this.publicKey)) {
                    return Vue.notify('error', 'Invalid public key is passed');
                }

                if (!Vue.validateHexString(this.secretKey, 64)) {
                    return Vue.notify('error', 'Invalid secret key is passed');
                }

                this.isSpinnerVisible = true;

                Vue.storage.set({
                    publicKey: this.publicKey,
                    secretKey: this.secretKey
                });

                this.$router.push({name: 'user'});
            },
            register: function() {
                const self = this;

                if (!this.name) {
                    return Vue.notify('error', 'The name is a required field');
                }

                this.keyPair = Exonum.keyPair();

                this.isSpinnerVisible = true;

                const TxCreateWallet = Exonum.newMessage({
                    size: 40,
                    network_id: this.NETWORK_ID,
                    protocol_version: this.PROTOCOL_VERSION,
                    service_id: this.SERVICE_ID,
                    message_id: this.TX_WALLET_ID,
                    fields: {
                        pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                        name: {type: Exonum.String, size: 8, from: 32, to: 40}
                    }
                });

                const data = {
                    pub_key: this.keyPair.publicKey,
                    name: this.name
                };

                const signature = TxCreateWallet.sign(this.keyPair.secretKey, data);

                this.$http.post('/api/services/cryptocurrency/v1/wallets/transaction', {
                    body: data,
                    network_id: 0,
                    protocol_version: 0,
                    service_id: 128,
                    message_id: 130,
                    signature: signature
                }).then(function() {
                    self.isSpinnerVisible = false;
                    self.isModalVisible = true;
                }).catch(function(error) {
                    self.isSpinnerVisible = false;
                    Vue.notify('error', error.toString());
                });
            },
            closeModal: function() {
                this.isModalVisible = false;
            },
            proceed: function() {
                Vue.storage.set(this.keyPair);

                this.$router.push({name: 'user'});
            }
        }
    }
</script>
