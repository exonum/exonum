<wallet>
    <div if="{ wallet && block }" class="text-center">
        <h2>${ wallet.balance }</h2>
        <h6>Block #<a href="/#blockchain/block/{ block.height }">{ block.height }</a></h6>
        <h6>{ moment(block.time / 1000000).format('HH:mm:ss, DD MMM YYYY') }</h6>
        <div if={ wallet.balance == 0 } class="alert alert-warning">
            <i class="glyphicon glyphicon-alert"></i> You haven't any money yet. Add some funds.
        </div>
        <div class="form-group">
            <button class="btn btn-lg btn-primary" disabled={ wallet.balance == 0 } onclick={ transfer }>Transfer</button>
            <a href="/#user/{ opts.publicKey }/add-funds" class="btn btn-lg btn-success">Add Funds</a>
        </div>
    </div>

    <virtual if={ transactions }>
        <legend class="text-center">Transactions history</legend>

        <table class="table table-striped">
            <thead>
            <tr>
                <th>Hash</th>
                <th>Description</th>
            </tr>
            </thead>
            <tbody>
            <tr each={ transactions }>
                <td>
                    <a href="/#blockchain/transaction/{ hash }">{ parent.truncate(hash, 16) }</a>
                </td>
                <td if={message_id === 130}>
                    create wallet
                </td>
                <td if={message_id === 129}>
                    add <strong>${ body.amount }</strong> to your wallet
                </td>
                <td if={message_id === 128 && body.from === parent.opts.publicKey}>
                    send <strong>${ body.amount }</strong> to <a href="/#user/{ body.to }">{ parent.truncate(body.to, 16) }</a>
                </td>
                <td if={message_id === 128 && body.to === parent.opts.publicKey}>
                    receive <strong>${ body.amount }</strong> from <a href="/#user/{ body.from }">{ parent.truncate(body.from, 16) }</a>
                </td>
            </tr>
            </tbody>
        </table>
    </virtual>

    <a class="btn btn-lg btn-block btn-default" href="/#">Log out</a>

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