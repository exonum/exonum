<add-funds>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#user/{ opts.publicKey }">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Add Funds</div>
        </div>
    </div>
    <div class="panel-body">
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
    </div>

    <script>
        var self = this;

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