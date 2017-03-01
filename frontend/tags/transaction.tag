<transaction>
    <virtual if={ transaction && transaction.message_id === 128 }>
        <table class="table text-center">
            <thead>
            <tr>
                <th class="text-center">From</th>
                <th class="text-center">To</th>
            </tr>
            </thead>
            <tbody>
            <tr>
                <td class="h4"><a href="/#user/{ transaction.body.from }">{ transaction.body.from }</a></td>
                <td class="h4"><a href="/#user/{ transaction.body.to }">{ transaction.body.to }</a></td>
            </tr>
            </tbody>
        </table>

        <div class="text-center">
            <h2>${ transaction.body.amount }</h2>
        </div>
    </virtual>

    <virtual if={ transaction && transaction.message_id === 129 }>
        <table class="table text-center">
            <thead>
            <tr>
                <th class="text-center">To</th>
            </tr>
            </thead>
            <tbody>
            <tr>
                <td class="h4"><a href="/#user/{ transaction.body.wallet }">{ transaction.body.wallet }</a></td>
            </tr>
            </tbody>
        </table>

        <div class="text-center">
            <h2>${ transaction.body.amount }</h2>
        </div>
    </virtual>

    <virtual if={ transaction && transaction.message_id === 130 }>
        <table class="table text-center">
            <thead>
            <tr>
                <th class="text-center">Name</th>
            </tr>
            </thead>
            <tbody>
            <tr>
                <td class="h4"><a href="/#user/{ transaction.body.pub_key }">{ transaction.body.name }</a></td>
            </tr>
            </tbody>
        </table>
    </virtual>

    <p if={ notFound } class="text-muted text-center">
        <i class="glyphicon glyphicon-ban-circle"></i> The server is not know the requested transaction. <br>Wait a few seconds and reload the page.
    </p>

    <a class="btn btn-lg btn-block btn-default" onclick={ back }>Back</a>

    <script>
        var self = this;

        back(e) {
            history.back();
        }

        $.ajax({
            method: 'GET',
            url: this.api.baseUrl + '/blockchain/transactions/' + this.opts.hash,
            success: function(data, textStatus, jqXHR) {
                if (data.type === 'FromHex') {
                    self.notFound = true;
                    self.update();
                    return;
                }

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
                self.update();
            },
            error: function(jqXHR, textStatus, errorThrown) {
                console.error(textStatus);
            }
        });
    </script>
</transaction>