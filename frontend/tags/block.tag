<block>
    <virtual if={ block }>
        <nav>
            <ul class="pager">
                <li class="previous"><a href="#" onclick={ previous }><span aria-hidden="true">&larr;</span> Previous block</a></li>
                <li class="next"><a href="#" onclick={ next }>Next block <span aria-hidden="true">&rarr;</span></a></a></li>
            </ul>
        </nav>

        <table class="table table-bordered">
            <tbody>
            <tr>
                <th>Hash</th>
                <td class="truncate">{ block.hash }</td>
            </tr>
            <tr>
                <th>Propose time</th>
                <td>{ moment(block.propose_time * 1000).fromNow() }</td>
            </tr>
            <tr>
                <th>Proposer</th>
                <td class="truncate">{ block.proposer }</td>
            </tr>
            <tr>
                <th>Tx hash</th>
                <td class="truncate">{ block.tx_hash }</td>
            </tr>
            <tr>
                <th>State hash</th>
                <td class="truncate">{ block.state_hash }</td>
            </tr>
            <tr>
                <th>Approved by</th>
                <td><strong>{ block.precommits_count }</strong> validators</td>
            </tr>
            </tbody>
        </table>

        <legend class="text-center no-border">Transactions</legend>

        <div class="custom-table">
            <div class="row">
                <div class="col-xs-6 custom-table-header">Hash</div>
                <div class="col-xs-6 custom-table-header">Description</div>
            </div>
            <div class="row" each={ block.txs }>
                <div class="col-xs-6 custom-table-column truncate">
                    { hash }
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 130 }>
                    Create { body.name } wallet
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 129 }>
                    <span class="truncate">{ body.wallet }</span> add funds of <strong>{ numeral(body.amount).format('$0,0') }</strong>
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 128 }>
                    <span class="truncate">{ body.from }</span> send <strong>{ numeral(body.amount).format('$0,0') }</strong> to <span class="truncate">{ body.to }</span>
                </div>
            </div>
        </div>
    </virtual>

    <virtual if={ notFound }>
        <p class="text-muted text-center">
            <i class="glyphicon glyphicon-ban-circle"></i> The server is not know the requested block. <br>Wait a few seconds and reload the page.
        </p>
    </virtual>

    <a class="btn btn-lg btn-block btn-default" href="#blockchain/">Back</a>

    <script>
        var self = this;
        var height = parseInt(this.opts.height);

        this.title = 'Block ' + height;

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