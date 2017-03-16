<transaction>
    <virtual if={ transaction }>
        <virtual if={ transaction.message_id === 128 }>
            <div class="custom-table text-center">
                <div class="row">
                    <div class="col-xs-6 custom-table-header-column">From</div>
                    <div class="col-xs-6 custom-table-header-column">To</div>
                </div>
                <div class="row">
                    <div class="col-xs-6 custom-table-column monospace expanded">
                        <a href="#user/{ transaction.body.from }">{ transaction.body.from }</a>
                    </div>
                    <div class="col-xs-6 custom-table-column monospace expanded">
                        <a href="#user/{ transaction.body.to }">{ transaction.body.to }</a>
                    </div>
                </div>
            </div>

            <div class="text-center">
                <h2>{ numeral(transaction.body.amount).format('$0,0') }</h2>
            </div>
        </virtual>

        <virtual if={ transaction.message_id === 129 }>
            <div class="custom-table text-center">
                <div class="row">
                    <div class="col-sm-12 custom-table-header-column">To</div>
                </div>
                <div class="row">
                    <div class="col-sm-12 custom-table-column monospace expanded">
                        <a href="#user/{ transaction.body.wallet }">{ transaction.body.wallet }</a>
                    </div>
                </div>
            </div>

            <div class="text-center">
                <h2>{ numeral(transaction.body.amount).format('$0,0') }</h2>
            </div>
        </virtual>

        <virtual if={ transaction.message_id === 130 }>
            <div class="custom-table text-center">
                <div class="row">
                    <div class="col-sm-12 custom-table-header-column">Name</div>
                </div>
                <div class="row">
                    <div class="col-sm-12 custom-table-column">
                        <a href="#user/{ transaction.body.pub_key }">{ transaction.body.name }</a>
                    </div>
                </div>
            </div>
        </virtual>
    </virtual>

    <p if={ notFound } class="text-muted text-center">
        <i class="glyphicon glyphicon-ban-circle"></i> The server is not know the requested transaction. <br>Wait a few seconds and reload the page.
    </p>

    <a class="btn btn-lg btn-block btn-default" onclick={ back }>Back</a>

    <script>
        var self = this;

        this.api.loadTransaction(this.opts.hash, function(data, textStatus, jqXHR) {
            if (data.type === 'FromHex') {
                self.notFound = true;
            } else {
                switch(data.message_id) {
                    case 128:
                        self.opts.titleObservable.trigger('change', 'Transfer Transaction');
                        break;
                    case 129:
                        self.opts.titleObservable.trigger('change', 'Add Funds Transaction');
                        break;
                    case 130:
                        self.opts.titleObservable.trigger('change', 'Create Wallet Transaction');
                        break;
                }
                self.transaction = data;
            }

            self.update();
        });

        back(e) {
            e.preventDefault();
            history.back();
        }
    </script>
</transaction>