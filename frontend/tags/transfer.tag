<transfer>
    <virtual if={ !succeed && !submitted }>
        <virtual if={ wallet && block }>
            <table class="table table-bordered">
                <tbody>
                <tr>
                    <th>Balance</th>
                    <td>{ numeral(wallet.balance).format('$0,0') }</td>
                </tr>
                <tr>
                    <th>Name</th>
                    <td>{ wallet.name }</td>
                </tr>
                <tr>
                    <th>Updated</th>
                    <td>{ moment(block.time / 1000000).fromNow() }</td>
                </tr>
                <tr>
                    <th>Block</th>
                    <td class="truncate"><a href="#blockchain/block/{ block.height }">{ block.height }</a></td>
                </tr>
                </tbody>
            </table>
        </virtual>

        <form class="form-horizontal" onsubmit={ submit }>
            <div class="form-group">
                <div class="col-sm-4 control-label">Reciever:</div>
                <div class="col-sm-8">
                    <select id="reciever" class="form-control">
                        <option each={ users } if={ publicKey !== opts.publicKey } value="{ publicKey }">{ name }</option>
                    </select>
                </div>
            </div>
            <div class="form-group">
                <div class="col-sm-4 control-label">Amount, $:</div>
                <div class="col-sm-8">
                    <input type="number" class="form-control" onkeyup={ edit }>
                </div>
            </div>
            <div class="form-group">
                <div class="col-sm-offset-4 col-sm-8">
                    <button type="submit" class="btn btn-lg btn-primary" disabled="{ !amount }">Transfer</button>
                    <a class="btn btn-lg btn-default" href="#user/{ opts.publicKey }">Back</a>
                </div>
            </div>
        </form>
    </virtual>

    <div if={ submitted && !succeed } class="text-center">
        <form class="form" onsubmit={ approve }>
            <p class="lead">Are you sure you want to send <strong>{ numeral(amount).format('$0,0') }</strong> to <a href="#user/{ reciever.publicKey }">{ reciever.name }</a>?</p>
            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-primary">Approve</button>
                <a class="btn btn-lg btn-default" href="#user/{ opts.publicKey }/transfer">Cancel</a>
            </div>
        </form>
    </div>

    <div if={ succeed } class="text-center">
        <p class="lead">Transfer approved. You've sent <strong>{ numeral(amount).format('$0,0') }</strong> to <a href="#user/{ reciever.publicKey }">{ reciever.name }</a>.</p>
        <div class="form-group">
            <a class="btn btn-lg btn-default" href="#user/{ opts.publicKey }">Back</a>
        </div>
    </div>

    <script>
        var self = this;

        this.title = 'Transfer';
        this.users = this.localStorage.getUsers();

        this.api.getWallet(self.opts.publicKey, function(data) {
            self.block = data.block;
            self.wallet = data.wallet;
            self.update();
        });

        edit(e) {
            if (e.target.value > 0 && e.target.value.toLowerCase().indexOf('e') === -1) {
                this.amount = e.target.value;
            } else {
                this.amount = 0;
            }
        }

        submit(e) {
            e.preventDefault();
            this.reciever = {
                publicKey: $('#reciever').val(),
                name: $('#reciever option:selected').text()
            }
            this.submitted = true;
            this.update();
        }

        approve(e) {
            e.preventDefault();
            var amount = self.amount.toString();
            var user = self.localStorage.getUser(self.opts.publicKey);
            var transaction = self.api.cryptocurrency.transferTransaction(amount, self.opts.publicKey, self.reciever.publicKey, user.secretKey);

            self.api.submitTransaction(transaction, function() {
                self.succeed = true;
                self.update();
            });
        }
    </script>
</transfer>