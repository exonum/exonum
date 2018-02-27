<history>
    <legend class="text-center no-border space-top">Transactions history</legend>

    <div class="custom-table">
        <div class="row">
            <div class="col-xs-4 custom-table-header-column">Hash</div>
            <div class="col-xs-5 custom-table-header-column">Description</div>
            <div class="col-xs-3 custom-table-header-column text-center">Status</div>
        </div>

        <div class="row" each={ opts.transactions }>
            <div class="col-xs-4 custom-table-column">
                <truncate val={ hash }></truncate>
            </div>
            <div class="col-xs-5 custom-table-column" if={ message_id === 130 }>
                Create wallet
            </div>
            <div class="col-xs-5 custom-table-column" if={ message_id === 129 }>
                Add <strong>{ numeral(body.amount).format('$0,0.00') }</strong> to your wallet
            </div>
            <div class="col-xs-5 custom-table-column" if={ message_id === 128 && body.from === opts.public_key }>
                Sent <strong>{ numeral(body.amount).format('$0,0.00') }</strong> to <truncate val={ body.to }></truncate>
            </div>
            <div class="col-xs-5 custom-table-column" if={ message_id === 128 && body.to === opts.public_key }>
                Received <strong>{ numeral(body.amount).format('$0,0.00') }</strong> from <truncate val={ body.from }></truncate>
            </div>
            <div class="col-xs-3 custom-table-column text-center">
                <i if={ status } class="glyphicon glyphicon-ok text-success"></i>
                <i if={ !status } class="glyphicon glyphicon-remove text-danger"></i>
            </div>
        </div>
    </div>
</history>
