<block>
    <!-- Block found state -->
    <nav>
        <ul class="pager">
            <li class="previous"><a href="#"><span aria-hidden="true">&larr;</span> Next block</a></li>
            <li class="next"><a href="#">Previous block <span aria-hidden="true">&rarr;</span></a></a></li>
        </ul>
    </nav>

    <ul class="list-group">
        <li class="list-group-item">
            <div class="row">
                <div class="col-md-3 text-muted">Hash:</div>
                <div class="col-md-9">e4f72dadcb1f8539e6ec1a5175fc0c6e7b813de2081389145fbbc35e5c7eea4a</div>
            </div>
            <div class="row">
                <div class="col-md-3 text-muted">Propose time:</div>
                <div class="col-md-9">14:53 20-03-2017</div>
            </div>
            <div class="row">
                <div class="col-md-3 text-muted">Proposer:</div>
                <div class="col-md-9">#5</div>
            </div>
            <div class="row">
                <div class="col-md-3 text-muted">Tx hash:</div>
                <div class="col-md-9">f1eb9742fdcf31caa249fdf2a9315c5071b555c5e42536aa08301a6070cbe4f1</div>
            </div>
            <div class="row">
                <div class="col-md-3 text-muted">State hash:</div>
                <div class="col-md-9">9a6e00ad6d80e1b4ab7bed81fd15a6abb2df3dfc36a036dffb42c094f4db34ff</div>
            </div>
        </li>
        <li class="list-group-item">

            <div class="checkbox">
                <label>
                    <input type="checkbox" value="" disabled checked>
                    Approved by <strong>7</strong> validators
                </label>
            </div>
            <div class="checkbox">
                <label>
                    <input type="checkbox" value="" disabled>
                    Anchored on Bitcoin blockchain
                </label>
            </div>
        </li>
    </ul>

    <legend class="text-center">Transactions</legend>

    <table class="table table-striped">
        <thead>
        <tr>
            <th>Date</th>
            <th>Description</th>
        </tr>
        </thead>
        <tbody>
        <tr>
            <td><a href="/#/blockchain/transaction/1389145fbba35e5c7eea4ae4f72dadcb1f8539e6ec1a5175fc0cee7b813de208">14:55 20-03-2017</a></td>
            <td>transfer <strong>$24.07</strong> from <a href="wallet.html">Tomas</a> to <a href="wallet.html">Jakob</a></td>
        </tr>
        <tr>
            <td><a href="/#/blockchain/transaction/0c6e7b813de2081d89145fbbc35e5c7eea4ae4f72dadcb1f8539e6ec1a5175fc">14:53 20-03-2017</a></td>
            <td>receive <strong>$16.83</strong> from <a href="wallet.html">Alex</a> from <a href="wallet.html">Bob</a></td>
        </tr>
        <tr>
            <td><a href="/#/blockchain/transaction/7eea4fe4f72dadcb1f8539e6ec1f5175fc0c6e7b813de2081389145fbbc35e5c">14:48 20-03-2017</a></td>
            <td>add <strong>$20.00</strong> to <a href="wallet.html">Alex</a>'s wallet</td>
        </tr>
        </tbody>
    </table>

    <a class="btn btn-lg btn-block btn-default" href="/#/blockchain">Back</a>

    <!-- Not found state -->
    <ul class="list-group">
        <li class="list-group-item text-center text-muted"><i class="glyphicon glyphicon-ban-circle"></i> The server is not know the requested block. <br>Wait a few seconds and reload the page.</li>
    </ul>

    <button class="btn btn-lg btn-block btn-primary">Reload page</button>
    <a class="btn btn-lg btn-block btn-default" href="blockchain.html">Back</a>

    <script>
        this.on('mount', function() {
            this.opts.title.trigger('change', 'Block');
        });
    </script>
</block>