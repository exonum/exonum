<transfer>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#user/{ opts.publicKey }">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Transfer Funds</div>
        </div>
    </div>
    <div class="panel-body">
        <virtual if={ wallet && block }>
            <wallet-summary wallet={ wallet } block={ block }></wallet-summary>
        </virtual>

        <form onsubmit={ submit }>
            <div class="form-group">
                <label class="control-label">Receiver:</label>
                <select id="receiver" class="form-control" disabled="{ users.length < 2 }">
                    <option each={ users } if={ publicKey !== opts.publicKey } value="{ publicKey }">{ name }</option>
                </select>
            </div>
            <div class="form-group">
                <label class="control-label">Amount, $:</label>
                <input type="number" class="form-control" onkeyup={ edit }>
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-block btn-primary" disabled="{ !amount }">Make a Transfer</button>
            </div>
        </form>
    </div>

    <script>
        var self = this;
        var user = self.storage.getUser();

        this.publicKey = user.publicKey;
        this.users = [];

        this.toggleLoading(true);
        this.service.getWallet(user.publicKey, function(block, wallet, transactions) {
            self.block = block;
            self.wallet = wallet;
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
            var user = self.storage.getUser();

            self.toggleLoading(true);
            self.service.transfer(amount, user.publicKey, receiver, user.secretKey, function() {
                self.toggleLoading(false);
                self.notify('success', 'Funds has been transferred.');
                route('/user');
            });
        }
    </script>
</transfer>