<register>
    <form if={ !succeed } class="form-horizontal" onsubmit={ register }>
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

    <div if={ succeed } class="text-center">
        <p class="lead">Wallet created! Login and manage the wallet.</p>
        <a class="btn btn-lg btn-block btn-default" href="/#user/{ publicKey }">Log in</a>
    </div>

    <script>
        var self = this;

        this.title = 'Register';

        edit(e) {
            this.text = e.target.value;
        }

        register(e) {
            var name = this.text;

            e.preventDefault();

            if (name) {
                var wallet = self.api.cryptocurrency.createWalletTransaction(name);

                self.api.submitTransaction(wallet.transaction, function() {
                    self.localStorage.addUser({
                        name: name,
                        publicKey: wallet.pair.publicKey,
                        secretKey: wallet.pair.secretKey
                    });
                    self.publicKey = wallet.pair.publicKey;
                    self.succeed = true;
                    self.update();
                });
            }
        }
    </script>
</register>