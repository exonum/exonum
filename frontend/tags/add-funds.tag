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
        var user = this.auth.getUser();

        this.toggleLoading(true);

        this.service.getWallet(user.publicKey, function(error, block, wallet, transactions) {
            self.toggleLoading(false);

            if (error) {
                self.notify('error', error.message +  ' An error occurred while trying to parse the wallet.', false);
                return;
            }

            self.block = block;
            self.wallet = wallet;
            self.update();
        });

        addFunds(e) {
            e.preventDefault();
            var amount = $(e.target).data('amount').toString();

            self.toggleLoading(true);
            self.service.addFunds(amount, user.publicKey, user.secretKey, function(error) {
                self.toggleLoading(false);

                if (error) {
                    self.notify('error', error.message);
                    return;
                }

                self.notify('success', 'Funds has been added into your account.');
                route('/user');
            });
        }
    </script>
</add-funds>
