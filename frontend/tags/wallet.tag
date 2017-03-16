<wallet>
    <virtual if="{ wallet && block }">

        <table class="table table-bordered">
            <tbody>
            <tr>
                <th>Balance</th>
                <td>{ numeral(wallet.balance).format('$0,0') }</td>
            </tr>
            <tr>
                <th>Name</th>
                <td>{ wallet.name }</td>
            </tr>
            <tr>
                <th>Updated</th>
                <td>{ moment(block.time / 1000000).fromNow() }</td>
            </tr>
            <tr>
                <th>Block</th>
                <td class="truncate"><a href="#blockchain/block/{ block.height }">{ block.height }</a></td>
            </tr>
            </tbody>
        </table>

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
    </virtual>

    <virtual if={ transactions }>
        <legend class="text-center no-border">Transactions history</legend>

        <div class="custom-table">
            <div class="row">
                <div class="col-xs-6 custom-table-header">Hash</div>
                <div class="col-xs-6 custom-table-header">Description</div>
            </div>
            <div class="row" each={ transactions }>
                <div class="col-xs-6 custom-table-column truncate">
                    { hash }
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 130 }>
                    Create wallet
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 129 }>
                    Add <strong>{ numeral(body.amount).format('$0,0') }</strong> to your wallet
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 128 && body.from === parent.opts.publicKey }>
                    Send <strong>{ numeral(body.amount).format('$0,0') }</strong> to <span class="truncate">{ body.to }</span>
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 128 && body.to === parent.opts.publicKey }>
                    Receive <strong>{ numeral(body.amount).format('$0,0') }</strong> from <span class="truncate">{ body.from }</span>
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
            route('/user/' + self.opts.publicKey + '/transfer');
        }
    </script>
</wallet>