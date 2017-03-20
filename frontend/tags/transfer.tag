<transfer>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#user/{ opts.publicKey }">
            &larr;
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Transfer</div>
        </div>
    </div>
    <div class="panel-body">
        <virtual if={ wallet && block }>
            <wallet-summary wallet={ wallet } block={ block }></wallet-summary>
        </virtual>

        <form onsubmit={ submit }>
            <div class="form-group">
                <label class="control-label">Receiver:</label>
                <select id="receiver" class="form-control">
                    <option each={ users } if={ publicKey !== opts.publicKey } value="{ publicKey }">{ name }</option>
                </select>
            </div>
            <div class="form-group">
                <label class="control-label">Amount, $:</label>
                <input type="number" class="form-control" onkeyup={ edit }>
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-block btn-primary" disabled="{ !amount }">Transfer</button>
            </div>
        </form>
    </div>

    <script>
        var self = this;

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
                self.notify('success', 'Funds has been transferred.');
                route('/user/' + self.opts.publicKey);
            });
        }
    </script>
</transfer>