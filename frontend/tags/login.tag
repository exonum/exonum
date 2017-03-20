<login>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Login</div>
        </div>
    </div>
    <div class="panel-body">
        <form onsubmit={ login }>
            <div class="form-group">
                <label class="control-label">Public key:</label>
                <input type="text" class="form-control" onkeyup="{ editPublicKey }">
            </div>
            <div class="form-group">
                <label class="control-label">Password:</label>
                <input type="text" class="form-control" onkeyup="{ editPassword }">
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-block btn-primary" disabled={ !name }>Login</button>
            </div>
        </form>
    </div>

    <script>
        var self = this;

        editPublicKey(e) {
            this.publicKey = e.target.value;
        }

        editPassword(e) {
            this.password = e.target.value;
        }

        login(e) {
            e.preventDefault();
//            var pair = self.api.cryptocurrency.keyPair();
//            var transaction = self.api.cryptocurrency.createWalletTransaction(pair.publicKey, self.name, pair.secretKey);
//
//            self.api.submitTransaction.call(self, transaction, pair.publicKey, function() {
//                self.localStorage.addUser({
//                    name: self.name,
//                    publicKey: pair.publicKey,
//                    secretKey: pair.secretKey
//                });
//                self.notify('success', 'Wallet has been created. Login and manage the wallet.');
//                route('/');
//            });
        }
    </script>
</login>