<blockchain>
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

    <a class="btn btn-lg btn-block btn-default" href="#">Back</a>

    <script>
        var self = this;

        this.title = 'Blockchain explorer';
        this.height = parseInt(this.opts.height);

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