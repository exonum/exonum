<wallet-summary>
    <div class="custom-dd">
        <div class="row">
            <div class="col-xs-6 custom-dd-column">
                <strong>Balance</strong>
            </div>
            <div class="col-xs-6 custom-dd-column">{ numeral(opts.wallet.balance).format('$0,0') }</div>
        </div>
        <div class="row">
            <div class="col-xs-6 custom-dd-column">
                <strong>Name</strong>
            </div>
            <div class="col-xs-6 custom-dd-column">{ opts.wallet.name }</div>
        </div>
        <div class="row">
            <div class="col-xs-6 custom-dd-column">
                <strong>Updated</strong>
            </div>
            <div class="col-xs-6 custom-dd-column">{ moment(opts.block.time / 1000000).fromNow() }</div>
        </div>
        <div class="row">
            <div class="col-xs-6 custom-dd-column">
                <strong>Block</strong>
            </div>
            <div class="col-xs-6 custom-dd-column">
                <a href="#blockchain/block/{ opts.block.height }">{ opts.block.height }</a>
            </div>
        </div>
    </div>

    <script></script>
</wallet-summary>