<add-funds>
    <virtual if={ wallet && block }>
        <wallet-summary wallet={ wallet } block={ block }></wallet-summary>
    </virtual>

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
    <div class="form-group">
        <a class="btn btn-lg btn-block btn-default" href="#user/{ opts.publicKey }">Back</a>
    </div>

    <script>
        var self = this;

        this.title = 'Add Funds';
        this.toggleLoading(true);
        this.api.getWallet(self.opts.publicKey, function(data) {
            self.block = data.block;
            self.wallet = data.wallet;
            self.update();
            self.toggleLoading(false);
        });

        addFunds(e) {
            e.preventDefault();
            var amount = $(e.target).data('amount').toString();
            var user = self.localStorage.getUser(self.opts.publicKey);
            var transaction = self.api.cryptocurrency.addFundsTransaction(amount, self.opts.publicKey, user.secretKey);

            self.api.submitTransaction.call(self, transaction, self.opts.publicKey, function() {
                self.notify('success', 'Funds has been added into your account.');
                route('/user/' + self.opts.publicKey);
            });
        }
    </script>
</add-funds>