<transfer>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#user">
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
                <input type="text" class="form-control" onkeyup={ editReceiver }>
            </div>
            <div class="form-group">
                <label class="control-label">Amount:</label>
                <div class="input-group">
                    <span class="input-group-addon">$</span>
                    <input type="number" class="form-control" onkeyup={ editAmount }>
                </div>
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-block btn-primary" disabled="{ !amount }">Make a Transfer</button>
            </div>
        </form>
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

        editReceiver(e) {
            this.receiver = e.target.value;
        }

        editAmount(e) {
            if (e.target.value > 0 && e.target.value.toLowerCase().indexOf('e') === -1) {
                this.amount = e.target.value;
            } else {
                this.amount = 0;
            }
        }

        submit(e) {
            e.preventDefault();
            var amount = self.amount.toString();

            self.toggleLoading(true);
            self.service.transfer(amount, user.publicKey, self.receiver, user.secretKey, function(error) {
                self.toggleLoading(false);

                if (error) {
                    self.notify('error', error.message);
                    return;
                }

                self.notify('success', 'Funds has been transferred into account.');

                route('/user');
            });
        }
    </script>
</transfer>
