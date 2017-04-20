<add-funds>
    <div class="panel-heading">
        <!-- TODO revert later -->
        <!--<a class="btn btn-default pull-left page-nav" href="#user">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>-->
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
        // TODO revert later
//        var user = self.storage.getUser();
//
//        this.publicKey = user.publicKey;
//
//        this.toggleLoading(true);
//        this.service.getWallet(user.publicKey, function(block, wallet, transactions) {
//            self.block = block;
//            self.wallet = wallet;
//            self.update();
//            self.toggleLoading(false);
//        });
//
//        addFunds(e) {
//            e.preventDefault();
//            var amount = $(e.target).data('amount').toString();
//            var user = self.storage.getUser();
//
//            self.toggleLoading(true);
//            self.service.addFunds(amount, user.publicKey, user.secretKey, function() {
//                self.toggleLoading(false);
//                self.notify('success', 'Funds has been added into your account.');
//                route('/user');
//            });
//        }

        this.toggleLoading(true);
        this.service.getWallet(self.opts.publicKey, function(block, wallet, transactions) {
            self.block = block;
            self.wallet = wallet;
            self.update();
            self.toggleLoading(false);
        });

        addFunds(e) {
            e.preventDefault();
            var amount = $(e.target).data('amount').toString();
            var user = self.storage.getUser(self.opts.publicKey);

            self.toggleLoading(true);
            self.service.addFunds(amount, self.opts.publicKey, user.secretKey, function() {
                self.toggleLoading(false);
                self.notify('success', 'Funds has been added into your account.');
                route('/user/' + self.opts.publicKey);
            });
        }
    </script>
</add-funds>