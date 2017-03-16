<block>
    <virtual if={ block }>
        <nav>
            <ul class="pager">
                <li class="previous"><a href="#" onclick={ previous }><span aria-hidden="true">&larr;</span> Previous block</a></li>
                <li class="next"><a href="#" onclick={ next }>Next block <span aria-hidden="true">&rarr;</span></a></a></li>
            </ul>
        </nav>

        <div class="custom-dd">
            <div class="row">
                <div class="col-xs-6 custom-dd-column">
                    <strong>Hash</strong>
                </div>
                <div class="col-xs-6 custom-dd-column">
                    <truncate val={ block.hash } digits=16></truncate>
                </div>
            </div>
            <div class="row">
                <div class="col-xs-6 custom-dd-column">
                    <strong>Propose time</strong>
                </div>
                <div class="col-xs-6 custom-dd-column">
                    { moment(block.propose_time * 1000).fromNow() }
                </div>
            </div>
            <div class="row">
                <div class="col-xs-6 custom-dd-column">
                    <strong>Proposer</strong>
                </div>
                <div class="col-xs-6 custom-dd-column">
                    { block.proposer }
                </div>
            </div>
            <div class="row">
                <div class="col-xs-6 custom-dd-column">
                    <strong>Tx hash</strong>
                </div>
                <div class="col-xs-6 custom-dd-column">
                    <truncate val={ block.tx_hash } digits=16></truncate>
                </div>
            </div>
            <div class="row">
                <div class="col-xs-6 custom-dd-column">
                    <strong>State hash</strong>
                </div>
                <div class="col-xs-6 custom-dd-column">
                    <truncate val={ block.state_hash } digits=16></truncate>
                </div>
            </div>
            <div class="row">
                <div class="col-xs-6 custom-dd-column">
                    <strong>Approved by</strong>
                </div>
                <div class="col-xs-6 custom-dd-column">
                    { block.precommits_count } validators
                </div>
            </div>
        </div>

        <legend class="text-center no-border space-top">Transactions</legend>

        <div class="custom-table">
            <div class="row">
                <div class="col-xs-6 custom-table-header">Hash</div>
                <div class="col-xs-6 custom-table-header">Description</div>
            </div>
            <div class="row" each={ block.txs }>
                <div class="col-xs-6 custom-table-column">
                    <truncate val={ hash } digits=16></truncate>
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 130 }>
                    Create { body.name } wallet
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 129 }>
                    <truncate val={ body.wallet }></truncate> add funds of <strong>{ numeral(body.amount).format('$0,0') }</strong>
                </div>
                <div class="col-xs-6 custom-table-column" if={ message_id === 128 }>
                    <truncate val={ body.from }></truncate> send <strong>{ numeral(body.amount).format('$0,0') }</strong> to <truncate val={ body.to }></truncate>
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