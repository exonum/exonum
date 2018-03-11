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
                            <li class="list-group-item" v-for="transaction in transactions">
                                <div class="row">
                                    <div class="col-sm-4"><code>{{ transaction.hash }}</code></div>
                                    <div class="col-sm-5" v-if="transaction.message_id == 130">Wallet created</div>
                                    <div class="col-sm-5" v-else-if="transaction.message_id == 129">
                                        <strong>{{ transaction.body.amount }}</strong> funds added
                                    </div>
                                    <div class="col-sm-5" v-else-if="transaction.message_id == 128 && transaction.body.from == publicKey">
                                        <strong>{{ transaction.body.amount }}</strong> sent to <code>{{ transaction.body.to }}</code>
                                    </div>
                                    <div class="col-sm-5" v-else-if="transaction.message_id == 128 && transaction.body.to == publicKey">
                                        <strong>{{ transaction.body.amount }}</strong> received from <code>{{ transaction.body.from }}</code>
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
    const Modal = require('../components/Modal.vue');
    const Spinner = require('../components/Spinner.vue');

    module.exports = {
        components: {
            Modal,
            Spinner
        },
        data: function() {
            return {
                name: undefined,
                publicKey: undefined,
                balance: undefined,
                height: undefined,
                amountToAdd: 10,
                receiver: undefined,
                amountToTransfer: undefined,
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
                    self.isSpinnerVisible = true;

                    self.$blockchain.addFunds(keyPair, self.amountToAdd).then(function() {
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
                    self.isSpinnerVisible = true;

                    self.$blockchain.transfer(keyPair, self.receiver, self.amountToTransfer).then(function() {
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

                    self.$blockchain.getWallet(keyPair).then(function(data) {
                        self.isSpinnerVisible = false;
                        self.name = data.wallet.name;
                        self.publicKey = keyPair.publicKey;
                        self.balance = data.wallet.balance;
                        self.height = data.block.height;
                        self.transactions = data.transactions;
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
