<transfer>
    <div class="text-center">
        <h2>$547.56</h2>
        <h6>Block <a href="block.html">#3248</a></h6>
    </div>

    <!-- Form -->
    <form class="form-horizontal" action="transfer-approve.html">
        <legend class="text-center">Transfer</legend>
        <div class="form-group">
            <div class="col-sm-4 control-label">Reciever:</div>
            <div class="col-sm-8">
                <select class="form-control">
                    <option>Alex</option>
                    <option>Bob</option>
                    <option>Tomas</option>
                </select>
            </div>
        </div>
        <div class="form-group has-error">
            <div class="col-sm-4 control-label">Amount, $:</div>
            <div class="col-sm-8">
                <input type="number" class="form-control">
            </div>
        </div>
        <div class="form-group">
            <div class="col-sm-offset-4 col-sm-8">
                <button type="submit" class="btn btn-lg btn-primary">Transfer</button>
                <a href="wallet.html" class="btn btn-lg btn-default">Back</a>
            </div>
        </div>
    </form>

    <!-- Approve -->
    <div class="text-center">
        <form action="transfer-approved.html" class="form">
            <p class="lead">Are you sure you want to send <strong>$24.56</strong> to <a href="wallet.html">Jakob</a>?</p>
            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-primary">Approve</button>
                <a href="wallet.html" class="btn btn-lg btn-default">Cancel</a>
            </div>
        </form>
    </div>

    <!-- Approved -->
    <div class="text-center">
        <p class="lead">Transfer approved. You've sent <strong>$24.56</strong> to <a href="wallet.html">Jakob</a>.</p>
        <div class="form-group">
            <a href="wallet.html" class="btn btn-lg btn-default">Back</a>
        </div>
    </div>

    <script>
        this.title = 'Transfer';
    </script>
</transfer>