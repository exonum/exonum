<register>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#dashboard">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Register</div>
        </div>
    </div>
    <div class="panel-body">
        <form onsubmit={ register }>
            <div class="form-group">
                <label class="control-label">Your name:</label>
                <input type="text" class="form-control" onkeyup="{ editName }">
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-block btn-primary" disabled={ !name }>Register a new wallet</button>
            </div>
        </form>
    </div>

    <script>
        var self = this;

        editName(e) {
            this.name = e.target.value;
        }

        register(e) {
            e.preventDefault();
            var pair = self.api.cryptocurrency.keyPair();
            var transaction = self.api.cryptocurrency.createWalletTransaction(pair.publicKey, self.name, pair.secretKey);

            self.api.submitTransaction.call(self, transaction, pair.publicKey, function() {
                self.localStorage.addUser({
                    name: self.name,
                    publicKey: pair.publicKey,
                    secretKey: pair.secretKey
                });
                self.notify('success', 'Wallet has been created. Login and manage the wallet.');
                route('/');
            });
        }
    </script>
</register>