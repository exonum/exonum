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
                <div class="col-xs-4 col-md-2 custom-table-header-column">Height</div>
                <div class="col-xs-4 col-md-3 custom-table-header-column">Prev.hash</div>
                <div class="col-sm-3 hidden-xs hidden-sm custom-table-header-column">State hash</div>
                <div class="col-sm-2 hidden-xs hidden-sm custom-table-header-column">Tx.hash</div>
                <div class="col-xs-4 col-md-2 custom-table-header-column">Round</div>
            </div>
            <div class="row" each={ blocks } onclick={ rowClick.bind(this, height) }>
                <div class="col-xs-4 col-md-2 custom-table-column">
                    { height }
                </div>
                <div class="col-xs-4 col-md-3 custom-table-column">
                    <truncate val={ prev_hash }></truncate>
                </div>
                <div class="col-sm-3 hidden-xs hidden-sm custom-table-column">
                    <truncate val={ state_hash }></truncate>
                </div>
                <div class="col-sm-2 hidden-xs hidden-sm custom-table-column">
                    <truncate val={ tx_hash }></truncate>
                </div>
                <div class="col-xs-4 col-md-2 custom-table-column text-center">
                    { propose_round }
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
        this.service.getBlocks(self.height + 1, function(blocks) {
            self.blocks = blocks;
            self.update();
            self.toggleLoading(false);
        });

        rowClick(height, e) {
            e.preventDefault();
            route('/blockchain/block/' + height);
        }

        more(e) {
            e.preventDefault();
            self.toggleLoading(true);
            this.service.getBlocks(self.blocks[self.blocks.length - 1].height, function(blocks) {
                self.blocks = self.blocks.concat(blocks);
                self.update();
                self.toggleLoading(false);
            });
        }

        refresh(e) {
            e.preventDefault();
            window.location.reload();
        }
    </script>
</blockchain>