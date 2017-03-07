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
                    <div class="col-md-9 truncate">{ block.hash }</div>
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
                    <div class="col-md-9 truncate">{ block.tx_hash }</div>
                </div>
                <div class="row">
                    <div class="col-md-3 text-muted">State hash:</div>
                    <div class="col-md-9 truncate">{ block.state_hash }</div>
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

        <legend class="text-center no-border">Transactions</legend>

        <div class="custom-table">
            <div class="row">
                <div class="col-xs-6 custom-table-header">Hash</div>
                <div class="col-xs-6 custom-table-header">Description</div>
            </div>
            <div class="row" each={ block.txs }>
                <div class="col-xs-6 custom-table-column truncate">
                    <a href="/#blockchain/transaction/{ hash }">{ hash }</a>
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 130 }>
                    create <a href="/#user/{ body.pub_key }">{ body.name }</a> wallet
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 129 }>
                    add <strong>${ body.amount }</strong> to <a href="/#user/{ body.wallet }" class="truncate">{ body.wallet }</a> wallet
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 128 }>
                    send <strong>${ body.amount }</strong> from <a href="/#user/{ body.from }" class="truncate">{ body.from }</a> to <a href="/#user/{ body.to }" class="truncate">{ body.to }</a>
                </div>
            </div>
        </div>
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