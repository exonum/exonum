<template>
    <div>
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
                                        <button class="btn btn-sm btn-outline-secondary ml-1" v-on:click="logout">Logout</button>
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
                                        {{ balance }}
                                        <button class="btn btn-sm btn-outline-success ml-1" v-on:click="openAddFundsModal">Add Funds</button>
                                        <button class="btn btn-sm btn-outline-primary ml-1" v-bind:disabled="!balance" v-on:click="openTransferModal">Transfer Funds</button>
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
                            <li class="list-group-item" v-for="tx in txs">
                                <div class="row">
                                    <div class="col-sm-4"><code>{{ tx.hash }}</code></div>
                                    <div class="col-sm-5" v-if="tx.message_id == 130">Wallet created</div>
                                    <div class="col-sm-5" v-else-if="tx.message_id == 129">
                                        <strong>{{ tx.body.amount }}</strong> funds added
                                    </div>
                                    <div class="col-sm-5" v-else-if="tx.message_id == 128 && tx.body.from == publicKey">
                                        <strong>{{ tx.body.amount }}</strong> sent to <code>{{ tx.body.to }}</code>
                                    </div>
                                    <div class="col-sm-5" v-else-if="tx.message_id == 128 && tx.body.to == publicKey">
                                        <strong>{{ tx.body.amount }}</strong> received from <code>{{ tx.body.from }}</code>
                                    </div>
                                    <div class="col-sm-3">
                                        <span v-if="tx.status" class="badge badge-success">executed</span>
                                        <span v-else class="badge badge-danger">failed</span>
                                    </div>
                                </div>
                            </li>
                        </ul>
                    </div>
                </div>
            </div>
        </div>

        <modal title="Add Funds" actionBtn="Add funds" v-bind:visible="isAddFundsModalVisible" v-on:close="closeAddFundsModal" v-on:submit="addFunds">
            <div class="form-group">
                <label class="d-block">Select amount to be added:</label>
                <div class="form-check form-check-inline" v-for="variant in variants">
                    <input class="form-check-input" type="radio" :id="variant.id" :value="variant.amount" :checked="amountToAdd == variant.amount" v-model="amountToAdd">
                    <label class="form-check-label" :for="variant.id">${{ variant.amount }}</label>
                </div>
            </div>
        </modal>

        <modal title="Transfer Funds" actionBtn="Transfer" v-bind:visible="isTransferModalVisible" v-on:close="closeTransferModal" v-on:submit="transfer">
            <div class="form-group">
                <label>Receiver:</label>
                <input type="text" class="form-control" placeholder="Enter public key" v-model="receiver">
            </div>
            <div class="form-group">
                <label>Amount:</label>
                <div class="input-group">
                    <div class="input-group-prepend">
                        <div class="input-group-text">$</div>
                    </div>
                    <input type="number" class="form-control" placeholder="Enter amount" min="1" v-model="amountToTransfer">
                </div>
            </div>
        </modal>
    </div>
</template>

<script>
    const Exonum = require('exonum-client');
    const Modal = require('../components/Modal.vue');
    const Spinner = require('../components/Spinner.vue');

    module.exports = {
        components: {
            Modal,
            Spinner
        },
        data: function() {
            return {
                amountToAdd: 10,
                receiver: '',
                amountToTransfer: '',
                isAddFundsModalVisible: false,
                isTransferModalVisible: false,
                isSpinnerVisible: false,
                variants: [
                    {id: 'ten', amount: 10},
                    {id: 'fifty', amount: 50},
                    {id: 'hundred', amount: 100}
                ]
            }
        },
        methods: {
            openAddFundsModal: function() {
                this.isAddFundsModalVisible = true;
            },

            closeAddFundsModal: function() {
                this.isAddFundsModalVisible = false;
            },

            addFunds: function() {
                const self = this;

                this.$storage.get().then(function(keyPair) {
                    const TxIssue = Exonum.newMessage({
                        size: 48,
                        network_id: self.NETWORK_ID,
                        protocol_version: self.PROTOCOL_VERSION,
                        service_id: self.SERVICE_ID,
                        message_id: self.TX_ISSUE_ID,
                        fields: {
                            wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            amount: {type: Exonum.Uint64, size: 8, from: 32, to: 40},
                            seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
                        }
                    });

                    const data = {
                        wallet: keyPair.publicKey,
                        amount: self.amountToAdd.toString(),
                        seed: Exonum.randomUint64()
                    };

                    const signature = TxIssue.sign(keyPair.secretKey, data);

                    self.isSpinnerVisible = true;

                    self.$http.post('/api/services/cryptocurrency/v1/wallets/transaction', {
                        network_id: self.NETWORK_ID,
                        protocol_version: self.PROTOCOL_VERSION,
                        service_id: self.SERVICE_ID,
                        message_id: self.TX_ISSUE_ID,
                        signature: signature,
                        body: data
                    }).then(function() {
                        self.isSpinnerVisible = false;
                        self.isAddFundsModalVisible = false;
                        self.$notify('success', 'Add funds transaction has been sent');
                    }).catch(function(error) {
                        self.isSpinnerVisible = false;
                        self.$notify('error', error.toString());
                    });
                }).catch(function(error) {
                    self.isAddFundsModalVisible = false;
                    self.$notify('error', error.toString());
                    self.logout();
                });
            },

            openTransferModal: function() {
                this.isTransferModalVisible = true;
            },

            closeTransferModal: function() {
                this.isTransferModalVisible = false;
            },

            transfer: function() {
                const self = this;

                if (!this.$validateHex(this.receiver)) {
                    return this.$notify('error', 'Invalid public key is passed');
                }

                this.$storage.get().then(function(keyPair) {
                    const TxTransfer = Exonum.newMessage({
                        size: 80,
                        network_id: self.NETWORK_ID,
                        protocol_version: self.PROTOCOL_VERSION,
                        service_id: self.SERVICE_ID,
                        message_id: self.TX_TRANSFER_ID,
                        fields: {
                            from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
                            amount: {type: Exonum.Int64, size: 8, from: 64, to: 72},
                            seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
                        }
                    });

                    const data = {
                        from: keyPair.publicKey,
                        to: self.receiver,
                        amount: self.amountToTransfer.toString(),
                        seed: Exonum.randomUint64()
                    };

                    const signature = TxTransfer.sign(keyPair.secretKey, data);

                    self.isSpinnerVisible = true;

                    self.$http.post('/api/services/cryptocurrency/v1/wallets/transaction', {
                        network_id: self.NETWORK_ID,
                        protocol_version: self.PROTOCOL_VERSION,
                        service_id: self.SERVICE_ID,
                        message_id: self.TX_TRANSFER_ID,
                        signature: signature,
                        body: data
                    }).then(function() {
                        self.isSpinnerVisible = false;
                        self.isTransferModalVisible = false;
                        self.$notify('success', 'Transfer transaction has been sent');
                    }).catch(function(error) {
                        self.isSpinnerVisible = false;
                        self.$notify('error', error.toString());
                    });
                }).catch(function(error) {
                    self.isTransferModalVisible = false;
                    self.$notify('error', error.toString());
                    self.logout();
                });
            },

            logout: function() {
                this.$storage.remove();
                this.$router.push({name: 'home'});
            }
        },
        mounted: function() {
            this.$nextTick(function () {
                const self = this;

                this.$storage.get().then(function(keyPair) {
                    self.isSpinnerVisible = true;

                    self.$http.get('/api/services/configuration/v1/configs/actual').then(function(response) {
                        const validators = response.data.config.validator_keys.map(function(validator) {
                            return validator.consensus_key;
                        });

                        self.$http.get('/api/services/cryptocurrency/v1/wallets/info?pubkey=' + keyPair.publicKey).then(function(response) {
                            // TODO verify proof response.data
                        }).catch(function(error) {
                            self.isSpinnerVisible = false;
                            self.$notify('error', error.toString());
                        });
                    }).catch(function(error) {
                        self.isSpinnerVisible = false;
                        self.$notify('error', error.toString());
                    });
                }).catch(function(error) {
                    self.$notify('error', error.toString());
                    self.logout();
                });
            })
        }
    }
</script>
