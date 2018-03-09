<template>
    <div>
        <modal title="Add Funds" actionBtn="Add funds" v-bind:visible="isAddFundsModalVisible" v-on:close="closeAddFundsModal" v-on:submit="addFunds">
            <div class="form-group">
                <label class="d-block">Select amount to be added:</label>
                <div class="form-check form-check-inline" v-for="variant in variants">
                    <input class="form-check-input" type="radio" :id="variant.id" :value="variant.amount" :checked="variant.checked" v-model="amountToAdd">
                    <label class="form-check-label" :for="id">${{ variant.amount }}</label>
                </div>
                <span>Picked: {{ amountToAdd }}</span>
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

    module.exports = {
        components: {
            Modal
        },
        data: function() {
            return {
                isAddFundsModalVisible: false,
                isTransferModalVisible: false,
                variants: [
                    {id: 'ten', amount: 10, checked: true},
                    {id: 'fifty', amount: 50},
                    {id: 'hundred', amount: 100}
                ]
            }
        },
        methods: {
            addFunds: function() {},
            closeAddFundsModal: function() {
                this.isAddFundsModalVisible = false;
            },
            transfer: function() {},
            closeTransferModal: function() {
                this.isTransferModalVisible = false;
            },
            logout: function() {
                this.$router.push({name: 'home'});
            }
        }
    }
</script>
