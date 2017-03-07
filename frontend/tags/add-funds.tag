<add-funds>
    <virtual if={ !succeed }>
        <virtual if={ wallet && block }>
            <div class="text-center">
                <h2 if={ wallet.balance }>${ wallet.balance }</h2>
                <h6 if={ block.height }>Block #<a href="/#blockchain/block/{ block.height }">{ block.height }</a></h6>
            </div>
        </virtual>

        <legend class="text-center">Add funds</legend>
        <button type="submit" class="btn btn-lg btn-block btn-success" onclick={ addFunds } data-amount="10">Add $10.00</button>
        <button type="submit" class="btn btn-lg btn-block btn-success" onclick={ addFunds } data-amount="50">Add $50.00</button>
        <button type="submit" class="btn btn-lg btn-block btn-success" onclick={ addFunds } data-amount="100">Add $100.00</button>
    </virtual>

    <p if={ succeed } class="lead text-center">Excellent! Funds will be transfered in a seconds.</p>

    <a class="btn btn-lg btn-block btn-default" href="/#user/{ opts.publicKey }">Back</a>

    <script>
        var self = this;

        this.title = 'Add Funds';

        this.api.getWallet(self.opts.publicKey, function(data) {
            self.block = data.block;
            self.wallet = data.wallet;
            self.update();
        });

        addFunds(e) {
            e.preventDefault();
            var amount = $(e.target).data('amount').toString();
            var user = self.localStorage.getUser(self.opts.publicKey);
            var transaction = self.api.cryptocurrency.addFundsTransaction(amount, self.opts.publicKey, user.secretKey);

            self.api.submitTransaction(transaction, function() {
                self.succeed = true;
                self.update();
            });
        }
    </script>
</add-funds>