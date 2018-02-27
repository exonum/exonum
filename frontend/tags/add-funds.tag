<add-funds>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#user">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Add Funds</div>
        </div>
    </div>

    <div class="panel-body">
        <p class="text-center">Select the amount to be added to your account:</p>

        <div class="form-group">
            <button type="submit" class="btn btn-lg btn-block btn-success" onclick={ addFunds } data-amount="10">Add $10.00</button>
        </div>

        <div class="form-group">
            <button type="submit" class="btn btn-lg btn-block btn-success" onclick={ addFunds } data-amount="50">Add $50.00</button>
        </div>

        <div class="form-group">
            <button type="submit" class="btn btn-lg btn-block btn-success" onclick={ addFunds } data-amount="100">Add $100.00</button>
        </div>
    </div>

    <script>
        var self = this;

        addFunds(e) {
            e.preventDefault();

            this.toggleLoading(true);

            this.auth.getUser().then(function(keyPair) {
                var TxIssue = Exonum.newMessage({
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

                var data = {
                    wallet: keyPair.publicKey,
                    amount: e.target.dataset.amount,
                    seed: Exonum.randomUint64()
                };

                var signature = TxIssue.sign(keyPair.secretKey, data);

                $.ajax({
                    method: 'POST',
                    url: '/api/services/cryptocurrency/v1/wallets/transaction',
                    contentType: 'application/json; charset=utf-8',
                    data: JSON.stringify({
                        body: data,
                        network_id: self.NETWORK_ID,
                        protocol_version: self.PROTOCOL_VERSION,
                        service_id: self.SERVICE_ID,
                        message_id: self.TX_ISSUE_ID,
                        signature: signature
                    }),
                    success: function() {
                        self.toggleLoading(true);

                        self.notify('success', 'Funds has been added into your account');

                        route('/user');
                    },
                    error: function(jqXHR, textStatus, errorThrown) {
                        throw errorThrown;
                    }
                });
            }).catch(function(error) {
                self.toggleLoading(true);

                self.notify('error', error);
            });
        }
    </script>
</add-funds>
