<wallet>
    <div class="panel-heading">
        <button class="btn btn-default pull-right page-nav" onclick={ refresh }>
            <i class="glyphicon glyphicon-refresh"></i>
            <span class="hidden-xs">Refresh</span>
        </button>
        <button class="btn btn-default pull-left page-nav" onclick={ logout }>
            <i class="glyphicon glyphicon-log-out"></i>
            <span class="hidden-xs">Logout</span>
        </button>
        <div class="panel-title page-title text-center">
            <div class="h4">Wallet</div>
        </div>
    </div>
    <div class="panel-body">
        <virtual if="{ wallet && block }">
            <wallet-summary wallet={ wallet } block={ block }></wallet-summary>

            <div class="form-group">
                <p class="text-center">Transfer your funds to another account:</p>
                <button class="btn btn-lg btn-block btn-primary" disabled={ wallet.balance== 0 } onclick={ transfer }>
                    Transfer Funds
                </button>
            </div>

            <div class="form-group">
                <p class="text-center">Add more finds to your account:</p>
                <a href="#user/{ publicKey }/add-funds" class="btn btn-lg btn-block btn-success">Add Funds</a>
            </div>
        </virtual>

        <virtual if={ transactions }>
            <legend class="text-center no-border space-top">Transactions history</legend>

            <div class="custom-table">
                <div class="row">
                    <div class="col-xs-4 custom-table-header-column">Hash</div>
                    <div class="col-xs-4 custom-table-header-column">Status</div>
                    <div class="col-xs-4 custom-table-header-column">Description</div>
                </div>
                <div class="row" each={ transactions }>
                    <div class="col-xs-4 custom-table-column">
                        <truncate val={ hash } digits=12></truncate>
                    </div>
                    <div class="col-xs-4 custom-table-column">
                        { status }
                    </div>
                    <div class="col-xs-4 custom-table-column" if={ message_id === 130 }>
                        Create wallet
                    </div>
                    <div class="col-xs-4 custom-table-column" if={ message_id === 129 }>
                        Add <strong>{ numeral(body.amount).format('$0,0.00') }</strong> to your wallet
                    </div>
                    <div class="col-xs-4 custom-table-column" if={ message_id === 128 && body.from === parent.publicKey }>
                        Send <strong>{ numeral(body.amount).format('$0,0.00') }</strong> to <truncate val={ body.to }></truncate>
                    </div>
                    <div class="col-xs-4 custom-table-column" if={ message_id === 128 && body.to === parent.publicKey }>
                        Receive <strong>{ numeral(body.amount).format('$0,0.00') }</strong> from <truncate val={ body.from }></truncate>
                    </div>
                </div>
            </div>
        </virtual>

        <div class="form-group">
            <p class="text-center">Explore all transactions:</p>
            <a href="#blockchain" class="btn btn-lg btn-block btn-default">Blockchain Explorer</a>
        </div>
    </div>

    <script>
        var self = this;
        var user = self.storage.getUser();

        this.publicKey = user.publicKey;

        this.toggleLoading(true);
        this.service.getWallet(user.publicKey, function(block, wallet, transactions) {
            self.block = block;
            self.wallet = wallet;
            self.transactions = transactions;
            self.update();
            self.toggleLoading(false);

            if (wallet.balance == 0) {
                self.notify('warning', 'You have not any money yet. Add some funds.');
            }
        });

        transfer(e) {
            e.preventDefault();
            route('/user/transfer');
        }

        refresh(e) {
            e.preventDefault();
            window.location.reload();
        }

        logout(e) {
            e.preventDefault();
            self.deleteUser();
            route('/dashboard');
        }
    </script>
</wallet>