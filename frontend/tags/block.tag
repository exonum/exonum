<block>
    <div class="panel-heading">
        <button class="btn btn-default pull-right page-nav" if={ !block } onclick={ refresh }>
            <i class="glyphicon glyphicon-refresh"></i>
            <span class="hidden-xs">Refresh</span>
        </button>
        <a class="btn btn-default pull-left page-nav" href="#blockchain/">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Block { opts.height }</div>
        </div>
    </div>
    <div class="panel-body">
        <virtual if={ block }>
            <nav>
                <ul class="pager">
                    <li class="previous"><a href="#" onclick={ previous }><span aria-hidden="true">&larr;</span> Previous<span class="hidden-xs"> block</span></a></li>
                    <li class="next"><a href="#" onclick={ next }>Next<span class="hidden-xs"> block</span> <span aria-hidden="true">&rarr;</span></a></a></li>
                </ul>
            </nav>

            <div class="custom-dd">
                <div class="row">
                    <div class="col-xs-6 custom-dd-column">
                        <strong>Hash</strong>
                    </div>
                    <div class="col-xs-6 custom-dd-column">
                        <truncate class="truncate" val={ block.hash }></truncate>
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
                        <truncate class="truncate" val={ block.tx_hash }></truncate>
                    </div>
                </div>
                <div class="row">
                    <div class="col-xs-6 custom-dd-column">
                        <strong>State hash</strong>
                    </div>
                    <div class="col-xs-6 custom-dd-column">
                        <truncate class="truncate" val={ block.state_hash }></truncate>
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
                    <div class="col-xs-4 custom-table-header-column">Hash</div>
                    <div class="col-xs-5 custom-table-header-column">Description</div>
                    <div class="col-xs-3 custom-table-header-column text-center">Status</div>
                </div>
                <div class="row" each={ block.txs }>
                    <div class="col-xs-4 custom-table-column">
                        <truncate val={ hash }></truncate>
                    </div>
                    <div class="col-xs-5 custom-table-column" if={ message_id === 130 }>
                        Create { body.name } wallet
                    </div>
                    <div class="col-xs-5 custom-table-column" if={ message_id === 129 }>
                        <truncate val={ body.wallet }></truncate> add funds of <strong>{ numeral(body.amount).format('$0,0.00') }</strong>
                    </div>
                    <div class="col-xs-5 custom-table-column" if={ message_id === 128 }>
                        <truncate val={ body.from }></truncate> send <strong>{ numeral(body.amount).format('$0,0.00') }</strong> to <truncate val={ body.to }></truncate>
                    </div>
                    <div class="col-xs-3 custom-table-column text-center">
                        <i if={ status } class="glyphicon glyphicon-ok text-success"></i>
                        <i if={ !status } class="glyphicon glyphicon-remove text-danger"></i>
                    </div>
                </div>
            </div>
        </virtual>

        <virtual if={ !block }>
            <p class="text-muted text-center">
                <i class="glyphicon glyphicon-ban-circle"></i> The server is not know the requested block. <br>Wait a few seconds and refresh the page.
            </p>
        </virtual>
    </div>

    <script>
        var self = this;
        var height = parseInt(this.opts.height);

        this.toggleLoading(true);
        this.service.getBlock(height, function(block) {
            self.block = block;
            self.update();
            self.toggleLoading(false);
        });

        previous(e) {
            e.preventDefault();
            route('/blockchain/block/' + (height - 1));
        }

        next(e) {
            e.preventDefault();
            route('/blockchain/block/' + (height + 1));
        }

        refresh(e) {
            e.preventDefault();
            window.location.reload();
        }
    </script>
</block>