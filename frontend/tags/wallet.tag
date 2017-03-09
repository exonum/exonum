<wallet>
    <div if="{ wallet && block }" class="text-center">
        <h2>${ wallet.balance }</h2>
        <h6>Block #<a href="#blockchain/block/{ block.height }">{ block.height }</a></h6>
        <h6>{ moment(block.time / 1000000).format('HH:mm:ss, DD MMM YYYY') }</h6>
        <div if={ wallet.balance == 0 } class="alert alert-warning">
            <i class="glyphicon glyphicon-alert"></i> You haven't any money yet. Add some funds.
        </div>
        <div class="form-group">
            <button class="btn btn-lg btn-primary" disabled={ wallet.balance == 0 } onclick={ transfer }>Transfer</button>
            <a href="#user/{ opts.publicKey }/add-funds" class="btn btn-lg btn-success">Add Funds</a>
        </div>
    </div>

    <virtual if={ transactions }>
        <legend class="text-center no-border">Transactions history</legend>

        <div class="custom-table">
            <div class="row">
                <div class="col-xs-6 custom-table-header">Hash</div>
                <div class="col-xs-6 custom-table-header">Description</div>
            </div>
            <div class="row" each={ transactions }>
                <div class="col-xs-6 custom-table-column truncate">
                    <a href="#blockchain/transaction/{ hash }">{ hash }</a>
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 130 }>
                    create wallet
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 129 }>
                    add <strong>${ body.amount }</strong> to your wallet
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 128 && body.from === parent.opts.publicKey }>
                    send <strong>${ body.amount }</strong> to <a href="#user/{ body.to }" class="truncate">{ body.to }</a>
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 128 && body.to === parent.opts.publicKey }>
                    receive <strong>${ body.amount }</strong> from <a href="#user/{ body.from }" class="truncate">{ body.from }</a>
                </div>
            </div>
        </div>
    </virtual>

    <a class="btn btn-lg btn-block btn-default" href="#">Log out</a>

    <script>
        var self = this;

        this.api.getWallet(self.opts.publicKey, function(data) {
            // update app title
            self.opts.titleObservable.trigger('change', data.wallet.name);

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