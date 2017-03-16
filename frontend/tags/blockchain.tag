<blockchain>
    <nav>
        <ul class="pager">
            <li class="previous"><a href="#" onclick={ previous }><span aria-hidden="true">&larr;</span> Older</a></li>
            <li class="next"><a href="#" onclick={ next }>Newer <span aria-hidden="true">&rarr;</span></a></a></li>
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
        <div class="row" each={ blocks } onclick={ block.bind(this, height) }>
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

    <nav>
        <ul class="pager">
            <li class="previous"><a href="#" onclick={ previous }><span aria-hidden="true">&larr;</span> Older</a></li>
            <li class="next"><a href="#" onclick={ next }>Newer <span aria-hidden="true">&rarr;</span></a></a></li>
        </ul>
    </nav>

    <a class="btn btn-lg btn-block btn-default" href="#">Back</a>

    <script>
        var self = this;

        this.title = 'Blockchain explorer';

        this.api.loadBlockchain(function(data) {
            self.blocks = data;
            self.update();
        });

        block(height, e) {
            e.preventDefault();
            route('/blockchain/block/' + height);
        }

        previous(e) {
            e.preventDefault();
            self.api.loadBlockchain(self.blocks[0].height - 9, function(data) {
                self.blocks = data;
                self.update();
            });
        }

        next(e) {
            e.preventDefault();
            self.api.loadBlockchain(self.blocks[0].height + 11, function(data) {
                self.blocks = data;
                self.update();
            });
        }
    </script>
</blockchain>