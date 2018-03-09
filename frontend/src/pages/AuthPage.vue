<template>
    <div>
        <div class="container">
            <div class="row justify-content-sm-center">
                <div class="col-md-6 col-md-offset-3">
                    <h1 class="mt-5 mb-4">Authorization</h1>

                    <ul class="nav nav-tabs mb-4">
                        <li class="nav-item">
                            <a class="nav-link active" href="#login" data-toggle="tab">Log in</a>
                        </li>
                        <li class="nav-item">
                            <a class="nav-link" href="#register" data-toggle="tab">Register</a>
                        </li>
                    </ul>

                    <div class="tab-content">
                        <div class="tab-pane fade show active" id="login">
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
                        </div>

                        <div class="tab-pane fade" id="register">
                            <form v-on:submit.prevent="register">
                                <div class="form-group">
                                    <label class="control-label">Name:</label>
                                    <input type="text" class="form-control" placeholder="Enter name" maxlength="260" v-model="name">
                                </div>
                                <button type="submit" class="btn btn-lg btn-block btn-primary">Register</button>
                            </form>
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <modal title="Wallet has been created" actionBtn="Log in" v-bind:visible="isModalVisible" v-on:close="closeModal" v-on:submit="proceed">
            <div class="alert alert-warning" role="alert">Save the key pair in a safe place. You will need it to log in to the demo next time.</div>
            <div class="form-group">
                <label>Public key:</label>
                <div><code>{{ publicKey }}</code></div>
            </div>
            <div class="form-group">
                <label>Secret key:</label>
                <div><code>{{ secretKey }}</code></div>
            </div>
        </modal>

        <spinner v-bind:visible="isSpinnerVisible"></spinner>
    </div>
</template>

<script>
    const Vue = require('vue');
    const Modal = require('../components/Modal.vue');
    const Spinner = require('../components/Spinner.vue');

    module.exports = {
        components: {
            Modal,
            Spinner
        },
        data: function() {
            return {
                isModalVisible: false,
                isSpinnerVisible: false
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

                // this.auth.setUser({
                //     publicKey: this.publicKey,
                //     secretKey: this.secretKey
                // });

                this.$router.push({name: 'user'});
            },
            register: function() {
                // this.keyPair = Exonum.keyPair();
                //
                // this.toggleLoading(true);
                //
                // var TxCreateWallet = Exonum.newMessage({
                //     size: 40,
                //     network_id: this.NETWORK_ID,
                //     protocol_version: this.PROTOCOL_VERSION,
                //     service_id: this.SERVICE_ID,
                //     message_id: this.TX_WALLET_ID,
                //     fields: {
                //         pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                //         name: {type: Exonum.String, size: 8, from: 32, to: 40}
                //     }
                // });
                //
                // var data = {
                //     pub_key: this.keyPair.publicKey,
                //     name: this.name
                // };
                //
                // var signature = TxCreateWallet.sign(this.keyPair.secretKey, data);
                //
                // $.ajax({
                //     method: 'POST',
                //     url: '/api/services/cryptocurrency/v1/wallets/transaction',
                //     contentType: 'application/json; charset=utf-8',
                //     data: JSON.stringify({
                //         body: data,
                //         network_id: 0,
                //         protocol_version: 0,
                //         service_id: 128,
                //         message_id: 130,
                //         signature: signature
                //     }),
                //     success: function() {
                //         self.toggleLoading(false);
                //
                //         $('#proceedModal').modal('show');
                //     },
                //     error: function(jqXHR, textStatus, errorThrown) {
                //         self.toggleLoading(false);
                //
                //         self.notify('error', errorThrown.toString());
                //     }
                // });
            },
            closeModal: function() {
                this.isModalVisible = false;
            },
            proceed: function() {
                // this.auth.setUser(this.keyPair);

                this.$router.push({name: 'user'});
            }
        }
    }
</script>
