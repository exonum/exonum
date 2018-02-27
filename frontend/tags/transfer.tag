<transfer>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#user">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Transfer Funds</div>
        </div>
    </div>

    <div class="panel-body">
        <form onsubmit={ submit }>
            <div class="form-group">
                <label class="control-label">Receiver:</label>
                <input type="text" class="form-control" onkeyup={ editReceiver }>
            </div>

            <div class="form-group">
                <label class="control-label">Amount:</label>
                <div class="input-group">
                    <span class="input-group-addon">$</span>
                    <input type="number" class="form-control" onkeyup={ editAmount }>
                </div>
            </div>

            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-block btn-primary" disabled="{ !amount }">Make a Transfer</button>
            </div>
        </form>
    </div>

    <script>
        var self = this;

        editReceiver(e) {
            this.receiver = e.target.value;
        }

        editAmount(e) {
            if (e.target.value > 0 && e.target.value.toLowerCase().indexOf('e') === -1) {
                this.amount = e.target.value;
            } else {
                this.amount = 0;
            }
        }

        submit(e) {
            e.preventDefault();

            this.toggleLoading(true);

            var keyPair = this.auth.getUser();

            var TxTransfer = Exonum.newMessage({
                size: 80,
                network_id: this.NETWORK_ID,
                protocol_version: this.PROTOCOL_VERSION,
                service_id: this.SERVICE_ID,
                message_id: this.TX_TRANSFER_ID,
                fields: {
                    from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                    to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
                    amount: {type: Exonum.Int64, size: 8, from: 64, to: 72},
                    seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
                }
            });

            var data = {
                from: keyPair.publicKey,
                to: this.receiver,
                amount: this.amount.toString(),
                seed: Exonum.randomUint64()
            };

            var signature = TxTransfer.sign(keyPair.secretKey, data);

            $.ajax({
                method: 'POST',
                url: '/api/services/cryptocurrency/v1/wallets/transaction',
                contentType: 'application/json; charset=utf-8',
                data: JSON.stringify({
                    body: data,
                    network_id: this.NETWORK_ID,
                    protocol_version: this.PROTOCOL_VERSION,
                    service_id: this.SERVICE_ID,
                    message_id: this.TX_TRANSFER_ID,
                    signature: signature
                }),
                success: function() {
                    self.toggleLoading(true);

                    self.notify('success', 'Funds has been transferred');

                    route('/user');
                },
                error: function(jqXHR, textStatus, errorThrown) {
                    callback(errorThrown);
                }
            });
        }
    </script>
</transfer>
