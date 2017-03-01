<block>
    <virtual if={ block }>
        <nav>
            <ul class="pager">
                <li class="previous"><a href="#" onclick={ previous }><span aria-hidden="true">&larr;</span> Previous block</a></li>
                <li class="next"><a href="#" onclick={ next }>Next block <span aria-hidden="true">&rarr;</span></a></a></li>
            </ul>
        </nav>

        <ul class="list-group">
            <li class="list-group-item">
                <div class="row">
                    <div class="col-md-3 text-muted">Hash:</div>
                    <div class="col-md-9">{ block.hash }</div>
                </div>
                <div class="row">
                    <div class="col-md-3 text-muted">Propose time:</div>
                    <div class="col-md-9">{ block.propose_time }</div>
                </div>
                <div class="row">
                    <div class="col-md-3 text-muted">Proposer:</div>
                    <div class="col-md-9">#{ block.proposer }</div>
                </div>
                <div class="row">
                    <div class="col-md-3 text-muted">Tx hash:</div>
                    <div class="col-md-9">{ block.tx_hash }</div>
                </div>
                <div class="row">
                    <div class="col-md-3 text-muted">State hash:</div>
                    <div class="col-md-9">{ block.state_hash }</div>
                </div>
            </li>
            <li class="list-group-item">
                <div class="checkbox">
                    <label>
                        <input type="checkbox" value="" disabled checked>
                        Approved by <strong>{ block.precommits_count }</strong> validators
                    </label>
                </div>
            </li>
        </ul>

        <legend class="text-center">Transactions</legend>

        <table class="table table-striped">
            <thead>
            <tr>
                <th>Date</th>
                <th>Description</th>
            </tr>
            </thead>
            <tbody>
            <tr each={ block.txs }>
                <td><a href="/#blockchain/transaction/{ hash }">{ hash }</a></td>
                <td if={message_id === 130}>
                    create <a href="/#user/{ body.pub_key }">{ body.name }</a> wallet
                </td>
                <td if={message_id === 129}>
                    add <strong>${ body.amount }</strong> to <a href="/#user/{ body.wallet }">{ body.wallet }</a> wallet
                </td>
                <td if={message_id === 128}>
                    send <strong>${ body.amount }</strong> from <a href="/#user/{ body.from }">{ body.from }</a> to <a href="/#user/{ body.to }">{ body.to }</a>
                </td>
            </tr>
            </tbody>
        </table>
    </virtual>

    <virtual if={ notFound }>
        <p class="text-muted text-center">
            <i class="glyphicon glyphicon-ban-circle"></i> The server is not know the requested block. <br>Wait a few seconds and reload the page.
        </p>
    </virtual>

    <a class="btn btn-lg btn-block btn-default" href="/#blockchain/">Back</a>

    <script>
        var self = this;

        var height = parseInt(this.opts.height);

        this.title = 'Block #' + height;

        previous(e) {
            e.preventDefault();
            route('/blockchain/block/' + (height - 1));
        }

        next(e) {
            e.preventDefault();
            route('/blockchain/block/' + (height + 1));
        }

        function getTransactionType(transaction) {
            switch (transaction.message_id) {
                case 128:
                    return Exonum.newMessage({
                        size: 80,
                        service_id: 128,
                        message_id: 128,
                        signature: transaction.signature,
                        fields: {
                            from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
                            amount: {type: Exonum.Int64, size: 8, from: 64, to: 72},
                            seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
                        }
                    });
                    break;
                case 129:
                    return Exonum.newMessage({
                        size: 48,
                        service_id: 128,
                        message_id: 129,
                        signature: transaction.signature,
                        fields: {
                            wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            amount: {type: Exonum.Int64, size: 8, from: 32, to: 40},
                            seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
                        }
                    });
                case 130:
                    return Exonum.newMessage({
                        size: 40,
                        service_id: 128,
                        message_id: 130,
                        signature: transaction.signature,
                        fields: {
                            pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            name: {type: Exonum.String, size: 8, from: 32, to: 40}
                        }
                    });
                    break;
            }
        }

        $.ajax({
            method: 'GET',
            url: this.api.baseUrl + '/blockchain/blocks/' + height,
            success: function(data, textStatus, jqXHR) {
                if (data == null) {
                    self.notFound = true;
                    self.update();
                    return;
                }
                self.block = data;
                for (var i in self.block.txs) {
                    self.block.txs[i].hash = Exonum.hash(self.block.txs[i].body, getTransactionType(self.block.txs[i]));
                }
                self.update();
            },
            error: function(jqXHR, textStatus, errorThrown) {
                console.error(textStatus);
            }
        });
    </script>
</block>