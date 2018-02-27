<register>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Register</div>
        </div>
    </div>

    <div class="panel-body">
        <form onsubmit={ submit }>
            <div class="form-group">
                <label class="control-label">Name:</label>
                <input type="text" class="form-control" onkeyup={ editName }>
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-block btn-primary" disabled={ !name }>Register a new wallet</button>
            </div>
        </form>
    </div>

    <div id="proceedModal" class="modal fade" tabindex="-1" role="dialog">
        <div class="modal-dialog" role="document">
            <div class="modal-content">
                <div class="modal-header">
                    <button type="button" class="close" data-dismiss="modal" aria-label="Close">
                        <span aria-hidden="true">&times;</span>
                    </button>
                    <h4 class="modal-title">Wallet has been created</h4>
                </div>
                <div class="modal-body" if={ keyPair }>
                    <p>Save the key pair in a safe place. You will need it to log into the demo next time.</p>
                    <div class="form-group">
                        <label>Public key:</label>
                        <pre><code>{ keyPair.publicKey }</code></pre>
                    </div>
                    <div class="form-group">
                        <label>Secret key:</label>
                        <pre><code>{ keyPair.secretKey }</code></pre>
                    </div>
                </div>
                <div class="modal-footer">
                    <button type="button" class="btn btn-default" data-dismiss="modal">Close</button>
                    <button type="button" class="btn btn-primary" onclick={ proceed }>Login</button>
                </div>
            </div>
        </div>
    </div>

    <script>
        var self = this;

        editName(e) {
            this.name = e.target.value;
        }

        submit(e) {
            e.preventDefault();

            this.keyPair = Exonum.keyPair();

            this.toggleLoading(true);

            var TxCreateWallet = Exonum.newMessage({
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

            var data = {
                pub_key: this.keyPair.publicKey,
                name: this.name
            };

            var signature = TxCreateWallet.sign(this.keyPair.secretKey, data);

            $.ajax({
                method: 'POST',
                url: '/api/services/cryptocurrency/v1/wallets/transaction',
                contentType: 'application/json; charset=utf-8',
                data: JSON.stringify({
                    body: data,
                    network_id: 0,
                    protocol_version: 0,
                    service_id: 128,
                    message_id: 130,
                    signature: signature
                }),
                success: function() {
                    self.toggleLoading(false);

                    $('#proceedModal').modal('show');
                },
                error: function(jqXHR, textStatus, errorThrown) {
                    self.toggleLoading(false);

                    self.notify('error', errorThrown);
                }
            });
        }

        proceed(e) {
            this.auth.setUser(this.keyPair);

            $('#proceedModal').modal('hide');

            route('/user');
        }
    </script>
</register>
