<truncate>
    <div class="input-group input-group-xs" onclick={ click }>
        <input type="text" class="form-control monospace" aria-label="Copy value to clipboard" value={ opts.val }>
        <div class="input-group-btn">
            <button type="button" class="btn btn-default" title="Copy to clipboard" data-toggle="tooltip" data-placement="bottom" data-clipboard-text={ opts.val }>
                <i class="glyphicon glyphicon-copy"></i>
            </button>
        </div>
    </div>

    <script>
        var self = this;

        this.on('mount', function() {
            var btn = this.root.getElementsByClassName('btn')[0];
            var $btn = $(btn);
            var $input = $(this.root.getElementsByClassName('form-control')[0]);
            var clipboard = new Clipboard(btn);

            clipboard.on('success', function() {
                self.notify('success', 'Copied to clipboard.');
                $input.focus().select();
            });

            $btn.tooltip();
        });

        click(e) {
            e.stopPropagation();
        }
    </script>
</truncate>
