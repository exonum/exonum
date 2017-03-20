<blockchain>
    <div class="panel-heading">
        <button class="btn btn-default pull-left page-nav">
            &larr;
            <span class="hidden-xs">Back</span>
        </button>
        <div class="panel-title page-title text-center">
            <div class="h4">Blockchain explorer</div>
        </div>
    </div>
    <div class="panel-body">
        <nav>
            <ul class="pager">
                <li class="previous" if={ hasPrevious }><a href="#" onclick={ previous }><span aria-hidden="true">&larr;</span> Older</a></li>
                <li class="next" if={ hasNext }><a href="#" onclick={ next }>Newer <span aria-hidden="true">&rarr;</span></a></li>
                <li class="next" if={ hasRefresh }><a href="#" onclick={ refresh }>Refresh</a></li>
            </ul>
        </nav>

        <div class="custom-table custom-table-hover">
            <div class="row">
                <div class="col-xs-4 col-sm-3 custom-table-header-column">Hash</div>
                <div class="col-xs-3 custom-table-header-column">Height</div>
                <div class="col-xs-2 col-sm-3 custom-table-header-column">
                    <span class="hidden-xs">Transactions</span>
                    <span class="visible-xs">Txs</span>
                </div>
                <div class="col-xs-3 custom-table-header-column">Date</div>
            </div>
            <div class="row" each={ blocks } onclick={ rowClick.bind(this, height) }>
                <div class="col-xs-4 col-sm-3 custom-table-column">
                    <truncate val={ hash } digits=8></truncate>
                </div>
                <div class="col-xs-3 custom-table-column">
                    { height }
                </div>
                <div class="col-xs-2 col-sm-3 custom-table-column">
                    { tx_count }
                </div>
                <div class="col-xs-3 custom-table-column">
                    { moment(propose_time * 1000).fromNow() }
                </div>
            </div>
        </div>
    </div>

    <script>
        var self = this;

        this.height = parseInt(this.opts.height);
        this.toggleLoading(true);
        this.api.loadBlockchain(self.height + 1, function(data) {
            self.blocks = data;

            // toggle previous button
            if (self.blocks[0].height > 9) {
                self.hasPrevious = true;
            }

            // toggle next and refresh buttons
            var newest = self.localStorage.getNewestHeight();
            if (isNaN(newest) || self.blocks[0].height >= newest)  {
                self.localStorage.setNewestHeight(self.blocks[0].height);
                self.hasRefresh = true;
            } else {
                self.hasNext = true;
            }

            self.update();
            self.toggleLoading(false);
        });

        rowClick(height, e) {
            e.preventDefault();
            route('/blockchain/block/' + height);
        }

        previous(e) {
            e.preventDefault();
            var height = self.blocks[0].height - 10;
            if (height < 9) {
                height = 9;
            }
            route('/blockchain/' + height);
        }

        next(e) {
            e.preventDefault();
            var height = self.blocks[0].height + 10;
            route('/blockchain/' + height);
        }

        refresh(e) {
            e.preventDefault();
            window.location.reload();
        }
    </script>
</blockchain>