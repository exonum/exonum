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
                <p class="text-center">Add more funds to your account:</p>
                <a href="#user/add-funds" class="btn btn-lg btn-block btn-success">Add Funds</a>
            </div>
        </virtual>

        <virtual if={ transactions }>
            <legend class="text-center no-border space-top">Transactions history</legend>

            <div class="custom-table">
                <div class="row">
                    <div class="col-xs-4 custom-table-header-column">Hash</div>
                    <div class="col-xs-5 custom-table-header-column">Description</div>
                    <div class="col-xs-3 custom-table-header-column text-center">Status</div>
                </div>
                <div class="row" each={ transactions }>
                    <div class="col-xs-4 custom-table-column">
                        <truncate val={ hash }></truncate>
                    </div>
                    <div class="col-xs-5 custom-table-column" if={ message_id === 130 }>
                        Create wallet
                    </div>
                    <div class="col-xs-5 custom-table-column" if={ message_id === 129 }>
                        Add <strong>{ numeral(body.amount).format('$0,0.00') }</strong> to your wallet
                    </div>
                    <div class="col-xs-5 custom-table-column" if={ message_id === 128 && body.from === wallet.pub_key }>
                        Send <strong>{ numeral(body.amount).format('$0,0.00') }</strong> to <truncate val={ body.to }></truncate>
                    </div>
                    <div class="col-xs-5 custom-table-column" if={ message_id === 128 && body.to === wallet.pub_key }>
                        Receive <strong>{ numeral(body.amount).format('$0,0.00') }</strong> from <truncate val={ body.from }></truncate>
                    </div>
                    <div class="col-xs-3 custom-table-column text-center">
                        <i if={ status } class="glyphicon glyphicon-ok text-success"></i>
                        <i if={ !status } class="glyphicon glyphicon-remove text-danger"></i>
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
        var user = self.auth.getUser();

        this.toggleLoading(true);

        this.service.getWallet(user.publicKey, function(error, block, wallet, transactions) {
            self.toggleLoading(false);

            if (error) {
                self.notify('error', error.message +  ' An error occurred while trying to parse the wallet.', false);
                return;
            }

            self.block = block;
            self.wallet = wallet;
            self.transactions = transactions;
            self.update();

            if (wallet && wallet.balance === 0) {
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
            self.auth.removeUser();
            route('/dashboard');
        }
    </script>
</wallet>
