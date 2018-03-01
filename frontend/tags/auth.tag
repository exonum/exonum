<auth>
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
                        <form onsubmit={ login }>
                            <div class="form-group">
                                <label class="control-label">Public key:</label>
                                <input type="text" class="form-control" placeholder="Enter public key" onkeyup={ editPublicKey }>
                            </div>
                            <div class="form-group">
                                <label class="control-label">Secret key:</label>
                                <input type="password" class="form-control" placeholder="Enter secret key" onkeyup={ editSecretKey }>
                            </div>
                            <button type="submit" class="btn btn-lg btn-block btn-primary" disabled={ !publicKey || !secretKey }>Log in</button>
                        </form>
                    </div>

                    <div class="tab-pane fade" id="register">
                        <form onsubmit={ register }>
                            <div class="form-group">
                                <label class="control-label">Name:</label>
                                <input type="text" class="form-control" placeholder="Enter name" onkeyup={ editName } maxlength="260">
                            </div>
                            <button type="submit" class="btn btn-lg btn-block btn-primary" disabled={ !name }>Register</button>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    </div>

    <div id="proceedModal" class="modal" tabindex="-1" role="dialog">
        <div class="modal-dialog" role="document">
            <div class="modal-content">
                <div class="modal-header">
                    <h5 class="modal-title">Wallet has been created</h5>
                    <button type="button" class="close" data-dismiss="modal" aria-label="Close">
                        <span aria-hidden="true">&times;</span>
                    </button>
                </div>

                <div class="modal-body" if={ keyPair }>
                    <div class="alert alert-warning" role="alert">Save the key pair in a safe place. You will need it to log in to the demo next time.</div>
                    <div class="form-group">
                        <label>Public key:</label>
                        <code>{ keyPair.publicKey }</code>
                    </div>
                    <div class="form-group">
                        <label>Secret key:</label>
                        <code>{ keyPair.secretKey }</code>
                    </div>
                </div>

                <div class="modal-footer">
                    <button type="button" class="btn btn-secondary" data-dismiss="modal">Close</button>
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

        editPublicKey(e) {
            this.publicKey = e.target.value;
        }

        editSecretKey(e) {
            this.secretKey = e.target.value;
        }

        login(e) {
            e.preventDefault();

            this.toggleLoading(true);

            this.auth.setUser({
                publicKey: this.publicKey,
                secretKey: this.secretKey
            });

            route('/user');
        }

        register(e) {
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

                    self.notify('error', errorThrown.toString());
                }
            });
        }

        proceed(e) {
            this.auth.setUser(this.keyPair);

            $('#proceedModal').modal('hide');

            route('/user');
        }
    </script>
</auth>
