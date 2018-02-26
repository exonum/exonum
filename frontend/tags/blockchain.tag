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
                <div class="col-xs-4 custom-table-header-column" title="Height">Height</div>
                <div class="col-xs-4 custom-table-header-column" title="Tx count">Tx count</div>
                <div class="col-xs-4 custom-table-header-column" title="State hash">State hash</div>
            </div>
            <div class="row" each={ blocks } onclick={ rowClick.bind(this, height) }>
                <div class="col-xs-4 custom-table-column">
                    { height }
                </div>
                <div class="col-xs-4 custom-table-column">
                    { tx_count }
                </div>
                <div class="col-xs-4 custom-table-column">
                    <truncate val={ state_hash }></truncate>
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
        var blocksPerPage = 10;

        this.toggleLoading(true);

        this.service.getBlocks(self.height + 1, blocksPerPage, function(error, blocks) {
            self.toggleLoading(false);

            if (error) {
                self.notify('error', error.name + ': ' + error.message);
                return;
            }

            self.blocks = blocks;
            self.update();
        });

        rowClick(height, e) {
            e.preventDefault();
            route('/blockchain/block/' + height);
        }

        more(e) {
            e.preventDefault();
            self.toggleLoading(true);
            this.service.getBlocks(self.blocks[self.blocks.length - 1].height, blocksPerPage, function(error, blocks) {
                self.toggleLoading(false);

                if (error) {
                    self.notify('error', error.message);
                    return;
                }

                self.blocks = self.blocks.concat(blocks);
                self.update();
            });
        }

        refresh(e) {
            e.preventDefault();
            window.location.reload();
        }
    </script>
</blockchain>
