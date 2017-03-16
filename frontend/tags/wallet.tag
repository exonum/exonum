<wallet>
    <virtual if="{ wallet && block }">

        <wallet-summary wallet={ wallet } block={ block }></wallet-summary>

        <div if={ wallet.balance == 0 } class="alert alert-warning text-center">
            <i class="glyphicon glyphicon-alert"></i> You haven't any money yet. Add some funds.
        </div>

        <div class="form-group">
            <button class="btn btn-lg btn-block btn-primary" disabled={ wallet.balance== 0 } onclick={ transfer }>
                Transfer
            </button>
        </div>

        <div class="form-group">
            <a href="#user/{ opts.publicKey }/add-funds" class="btn btn-lg btn-block btn-success">Add Funds</a>
        </div>

        <div class="form-group">
            <button class="btn btn-lg btn-block btn-default" onclick={ refresh }>
                Refresh
            </button>
        </div>
    </virtual>

    <virtual if={ transactions }>
        <legend class="text-center no-border space-top">Transactions history</legend>

        <div class="custom-table">
            <div class="row">
                <div class="col-xs-6 custom-table-header-column">Hash</div>
                <div class="col-xs-6 custom-table-header-column">Description</div>
            </div>
            <div class="row" each={ transactions }>
                <div class="col-xs-6 custom-table-column">
                    <truncate val={ hash } digits=12></truncate>
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 130 }>
                    Create wallet
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 129 }>
                    Add <strong>{ numeral(body.amount).format('$0,0') }</strong> to your wallet
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 128 && body.from === parent.opts.publicKey }>
                    Send <strong>{ numeral(body.amount).format('$0,0') }</strong> to <truncate val={ body.to }></truncate>
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 128 && body.to === parent.opts.publicKey }>
                    Receive <strong>{ numeral(body.amount).format('$0,0') }</strong> from <truncate val={ body.from }></truncate>
                </div>
            </div>
        </div>
    </virtual>

    <a class="btn btn-lg btn-block btn-default" href="#">Log out</a>

    <script>
        var self = this;

        this.title = 'Your wallet';

        this.api.getWallet(self.opts.publicKey, function(data) {
            self.block = data.block;
            self.wallet = data.wallet;
            self.transactions = data.transactions;
            self.update();
        });

        transfer(e) {
            e.preventDefault();
            route('/user/' + self.opts.publicKey + '/transfer');
        }

        refresh(e) {
            e.preventDefault();
            window.location.reload();
        }
    </script>
</wallet>