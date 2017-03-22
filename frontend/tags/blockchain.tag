<blockchain>
    <div class="panel-heading">
        <button class="btn btn-default pull-right page-nav" onclick={ refresh }>
            <i class="glyphicon glyphicon-refresh"></i>
            <span class="hidden-xs">Refresh</span>
        </button>
        <a class="btn btn-default pull-left page-nav" href="#dashboard">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Blockchain Explorer</div>
        </div>
    </div>
    <div class="panel-body">
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

        <div class="form-group">
            <button class="btn btn-lg btn-block btn-primary" onclick={ more }>
                Load more
            </button>
        </div>
    </div>

    <script>
        var self = this;

        this.toggleLoading(true);
        this.api.loadBlockchain(self.height + 1, function(data) {
            self.blocks = data;
            self.update();
            self.toggleLoading(false);
        });

        rowClick(height, e) {
            e.preventDefault();
            route('/blockchain/block/' + height);
        }

        refresh(e) {
            e.preventDefault();
            window.location.reload();
        }

        more(e) {
            e.preventDefault();
            self.toggleLoading(true);
            this.api.loadBlockchain(self.blocks[self.blocks.length - 1].height, function(data) {
                self.blocks = self.blocks.concat(data);
                self.update();
                self.toggleLoading(false);
            });
        }
    </script>
</blockchain>