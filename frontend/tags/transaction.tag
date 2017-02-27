<transaction>

    <!-- Add funds -->
    <table class="table text-center">
        <thead>
        <tr>
            <th class="text-center">To</th>
        </tr>
        </thead>
        <tbody>
        <tr>
            <td class="h4"><a href="/#/user/a89036a469fea208bab8d69d164488cd273b70bb6f0e9b85d52cfbc5d4c98217">Tomas</a></td>
        </tr>
        </tbody>
    </table>

    <div class="text-center">
        <h2>$10.00</h2>
        <p class="text-muted">14:55 20-03-2017</p>
    </div>

    <ul class="list-group">
        <li class="list-group-item text-center text-success"><i class="glyphicon glyphicon-ok"></i> Transaction is approved and commited.</li>
        <li class="list-group-item text-center text-warning"><i class="glyphicon glyphicon-hourglass"></i> Transaction is found in the pool and is not finalized yet. Wait a few seconds and reload the page.</li>
        <li class="list-group-item">
            <div class="checkbox">
            </div>
            <div class="checkbox">
                <label>
                    <input type="checkbox" value="" disabled checked>
                    Approved by <strong>7</strong> validators
                </label>
            </div>
            <div class="checkbox">
                <label>
                    <input type="checkbox" value="" disabled checked>
                    Commited with block <a href="block.html">#3248</a>
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

    <button class="btn btn-lg btn-block btn-primary">Reload page</button>
    <a class="btn btn-lg btn-block btn-default" href="wallet.html">Back</a>

    <!-- Transfer -->
    <table class="table text-center">
        <thead>
        <tr>
            <th class="text-center">From</th>
            <th class="text-center">To</th>
        </tr>
        </thead>
        <tbody>
        <tr>
            <td class="h4"><a href="/#/user/a89036a469fea208bab8d69d164488cd273b70bb6f0e9b85d52cfbc5d4c98217">Jakob</a></td>
            <td class="h4"><a href="/#/user/a89036a469fea208bab8d69d164488cd273b70bb6f0e9b85d52cfbc5d4c98217">Tomas</a></td>
        </tr>
        </tbody>
    </table>

    <div class="text-center">
        <h2>$500.00</h2>
        <p class="text-muted">14:55 20-03-2017</p>
    </div>

    <ul class="list-group">
        <li class="list-group-item text-center text-success"><i class="glyphicon glyphicon-ok"></i> Transaction is approved and commited.</li>
        <li class="list-group-item text-center text-warning"><i class="glyphicon glyphicon-hourglass"></i> Transaction is found in the pool and is not finalized yet. Wait a few seconds and reload the page.</li>
        <li class="list-group-item text-center text-danger"><i class="glyphicon glyphicon glyphicon-remove"></i>  Transaction is canceled because <a href="wallet.html">Jakob</a> has not enough money.</li>
        <li class="list-group-item">
            <div class="checkbox">
                <label>
                    <input type="checkbox" value="" disabled checked>
                    Signed by <a href="wallet.html">Jakob</a>'s private key
                </label>
            </div>
            <div class="checkbox">
                <label>
                    <input type="checkbox" value="" disabled checked>
                    Approved by <strong>7</strong> validators
                </label>
            </div>
            <div class="checkbox">
                <label>
                    <input type="checkbox" value="" disabled checked>
                    Canceled by <strong>7</strong> validators
                </label>
            </div>
            <div class="checkbox">
                <label>
                    <input type="checkbox" value="" disabled checked>
                    Commited with block <a href="block.html">#3248</a>
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

    <button class="btn btn-lg btn-block btn-primary">Reload page</button>
    <a class="btn btn-lg btn-block btn-default" href="wallet.html">Back</a>

    <!-- Not found -->
    <ul class="list-group">
        <li class="list-group-item text-center text-muted"><i class="glyphicon glyphicon-ban-circle"></i> The server is not know the requested transaction. <br>Wait a few seconds and reload the page.</li>
    </ul>

    <button class="btn btn-lg btn-block btn-primary">Reload page</button>
    <a class="btn btn-lg btn-block btn-default" href="wallet.html">Back</a>

</transaction>