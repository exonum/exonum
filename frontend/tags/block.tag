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
                        <strong>Height</strong>
                    </div>
                    <div class="col-xs-6 custom-dd-column">
                        { block.height }
                    </div>
                </div>
                <div class="row">
                    <div class="col-xs-6 custom-dd-column">
                        <strong>Tx count</strong>
                    </div>
                    <div class="col-xs-6 custom-dd-column">
                        { block.tx_count }
                    </div>
                </div>
                <div class="row">
                    <div class="col-xs-6 custom-dd-column">
                        <strong>Proposer ID</strong>
                    </div>
                    <div class="col-xs-6 custom-dd-column">
                        { block.proposer_id }
                    </div>
                </div>
                <div class="row">
                    <div class="col-xs-6 custom-dd-column">
                        <strong>Previous hash</strong>
                    </div>
                    <div class="col-xs-6 custom-dd-column">
                        <truncate class="truncate" val={ block.prev_hash }></truncate>
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
            </div>

            <virtual if={ txs && txs.length > 0 }>
                <legend class="text-center no-border space-top">Transactions</legend>

                <div class="custom-table">
                    <div class="row">
                        <div class="col-xs-12 custom-table-header-column">Hash</div>
                    </div>
                    <div class="row" each={ hash in txs } onclick={ rowClick.bind(this, hash) }>
                        <div class="col-xs-8 custom-table-column">
                            <truncate class="truncate" val={ hash }></truncate>
                        </div>
                        <div class="col-xs-4 custom-table-column">
                            <button class="btn btn-default btn-xs">Get Details</button>
                        </div>
                    </div>
                </div>
            </virtual>
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

        this.service.getBlock(height, function(error, response) {
            self.toggleLoading(false);

            if (error) {
                self.notify('error', error.message);
                return;
            }

            self.block = response.block;
            self.txs = response.txs;
            self.update();
        });

        rowClick(hash, e) {
            e.preventDefault();
            route('/blockchain/transaction/' + hash);
        }

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
