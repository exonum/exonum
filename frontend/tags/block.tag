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
                    <div class="col-md-9">{ truncate(block.hash, 24) }</div>
                </div>
                <div class="row">
                    <div class="col-md-3 text-muted">Propose time:</div>
                    <div class="col-md-9">{ moment(block.propose_time * 1000).format('HH:mm:ss, DD MMM YYYY') }</div>
                </div>
                <div class="row">
                    <div class="col-md-3 text-muted">Proposer:</div>
                    <div class="col-md-9">#{ block.proposer }</div>
                </div>
                <div class="row">
                    <div class="col-md-3 text-muted">Tx hash:</div>
                    <div class="col-md-9">{ truncate(block.tx_hash, 24) }</div>
                </div>
                <div class="row">
                    <div class="col-md-3 text-muted">State hash:</div>
                    <div class="col-md-9">{ truncate(block.state_hash, 24) }</div>
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
                <th>Hash</th>
                <th>Description</th>
            </tr>
            </thead>
            <tbody>
            <tr each={ block.txs }>
                <td><a href="/#blockchain/transaction/{ hash }">{ parent.truncate(hash, 16) }</a></td>
                <td if={message_id === 130}>
                    create <a href="/#user/{ body.pub_key }">{ body.name }</a> wallet
                </td>
                <td if={message_id === 129}>
                    add <strong>${ body.amount }</strong> to <a href="/#user/{ body.wallet }">{ parent.truncate(body.wallet, 16) }</a> wallet
                </td>
                <td if={message_id === 128}>
                    send <strong>${ body.amount }</strong> from <a href="/#user/{ body.from }">{ parent.truncate(body.from, 12) }</a> to <a href="/#user/{ body.to }">{ parent.truncate(body.to, 12) }</a>
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

        this.api.loadBlock(height, function(data) {
            if (data == null) {
                self.notFound = true;
            } else {
                self.block = data;
            }

            self.update();
        });

        previous(e) {
            e.preventDefault();
            route('/blockchain/block/' + (height - 1));
        }

        next(e) {
            e.preventDefault();
            route('/blockchain/block/' + (height + 1));
        }
    </script>
</block>