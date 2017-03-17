<transfer>
    <virtual if={ wallet && block }>
        <wallet-summary wallet={ wallet } block={ block }></wallet-summary>
    </virtual>

    <form class="form-horizontal" onsubmit={ submit }>
        <div class="form-group">
            <div class="col-sm-4 control-label">Receiver:</div>
            <div class="col-sm-8">
                <select id="receiver" class="form-control">
                    <option each={ users } if={ publicKey !== opts.publicKey } value="{ publicKey }">{ name }</option>
                </select>
            </div>
        </div>
        <div class="form-group">
            <div class="col-sm-4 control-label">Amount, $:</div>
            <div class="col-sm-8">
                <input type="number" class="form-control" onkeyup={ edit }>
            </div>
        </div>
        <div class="form-group">
            <div class="col-sm-offset-4 col-sm-8">
                <button type="submit" class="btn btn-lg btn-primary" disabled="{ !amount }">Transfer</button>
                <a class="btn btn-lg btn-default" href="#user/{ opts.publicKey }">Back</a>
            </div>
        </div>
    </form>

    <script>
        var self = this;

        this.title = 'Transfer';
        this.users = this.localStorage.getUsers();
        this.toggleLoading(true);
        this.api.getWallet(self.opts.publicKey, function(data) {
            self.block = data.block;
            self.wallet = data.wallet;
            self.update();
            self.toggleLoading(false);
        });

        edit(e) {
            if (e.target.value > 0 && e.target.value.toLowerCase().indexOf('e') === -1) {
                this.amount = e.target.value;
            } else {
                this.amount = 0;
            }
        }

        submit(e) {
            e.preventDefault();

            var amount = self.amount.toString();
            var receiver = $('#receiver').val();
            var user = self.localStorage.getUser(self.opts.publicKey);
            var transaction = self.api.cryptocurrency.transferTransaction(amount, self.opts.publicKey, receiver, user.secretKey);

            self.api.submitTransaction.call(self, transaction, self.opts.publicKey, function() {
                self.notify('success', 'Transfer approved. Funds will be transfered in a seconds.');
                route('/user/' + self.opts.publicKey);
            });
        }
    </script>
</transfer>