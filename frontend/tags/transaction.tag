<transaction>
    <virtual if={ transaction }>
        <virtual if={ transaction.message_id === 128 }>
            <table class="table text-center">
                <thead>
                <tr>
                    <th class="text-center">From</th>
                    <th class="text-center">To</th>
                </tr>
                </thead>
                <tbody>
                <tr>
                    <td class="h4"><a href="/#user/{ transaction.body.from }">{ truncate(transaction.body.from, 16) }</a></td>
                    <td class="h4"><a href="/#user/{ transaction.body.to }">{ truncate(transaction.body.to, 16) }</a></td>
                </tr>
                </tbody>
            </table>

            <div class="text-center">
                <h2>${ transaction.body.amount }</h2>
            </div>
        </virtual>

        <virtual if={ transaction.message_id === 129 }>
            <table class="table text-center">
                <thead>
                <tr>
                    <th class="text-center">To</th>
                </tr>
                </thead>
                <tbody>
                <tr>
                    <td class="h4"><a href="/#user/{ transaction.body.wallet }">{ truncate(transaction.body.wallet, 16) }</a></td>
                </tr>
                </tbody>
            </table>

            <div class="text-center">
                <h2>${ transaction.body.amount }</h2>
            </div>
        </virtual>

        <virtual if={ transaction.message_id === 130 }>
            <table class="table text-center">
                <thead>
                <tr>
                    <th class="text-center">Name</th>
                </tr>
                </thead>
                <tbody>
                <tr>
                    <td class="h4"><a href="/#user/{ transaction.body.pub_key }">{ truncate(transaction.body.name, 16) }</a></td>
                </tr>
                </tbody>
            </table>
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
            history.back();
        }
    </script>
</transaction>