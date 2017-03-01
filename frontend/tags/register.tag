<register>
    <form class="form-horizontal" onsubmit="{ register }">
        <legend class="text-center">Create wallet</legend>
        <div class="form-group">
            <div class="col-sm-4 control-label">Your name:</div>
            <div class="col-sm-8">
                <input type="text" class="form-control" onkeyup="{ edit }">
            </div>
        </div>
        <div class="form-group">
            <div class="col-sm-offset-4 col-sm-8">
                <button type="submit" class="btn btn-lg btn-primary" disabled={ !text }>Create wallet</button>
                <a href="/#" class="btn btn-lg btn-default">Back</a>
            </div>
        </div>
    </form>

    <script>
        this.title = 'Register';

        edit(e) {
            this.text = e.target.value;
        }

        register(e) {
            e.preventDefault();

            if (this.text) {
                // TODO move outside
                var TxCreateWallet = Exonum.newMessage({
                    size: 40,
                    service_id: 128,
                    message_id: 130,
                    fields: {
                        pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                        name: {type: Exonum.String, size: 8, from: 32, to: 40}
                    }
                });
                var pair = Exonum.keyPair();
                var data = {
                    pub_key: pair.publicKey,
                    name: this.text
                };
                var signature = Exonum.sign(data, TxCreateWallet, pair.secretKey);

                $.ajax({
                    method: 'POST',
                    url: this.api.baseUrl + '/wallets/transaction',
                    contentType: 'application/json',
                    data: JSON.stringify({
                        service_id: 128,
                        message_id: 130,
                        body: data,
                        signature: signature
                    }),
                    success: function(data, textStatus, jqXHR) {
                        route('/user/' + pair.publicKey);
                        var users = JSON.parse(window.localStorage.getItem('users'));
                        if (!users) {
                            users = [];
                        }
                        users.push(pair);
                        window.localStorage.setItem('users', JSON.stringify(users));
                    },
                    error: function(jqXHR, textStatus, errorThrown) {
                        console.error(textStatus);
                    }
                });
            }
        }
    </script>
</register>