<clipboard>
    <button type="button" class="btn btn-xs btn-default" data-clipboard-text={ opts.val } onclick={ click }>
        <i class="glyphicon glyphicon-copy"></i>
    </button>

    <script>
        var self = this;

        this.on('mount', function() {
            var clipboard = new Clipboard(this.root.getElementsByClassName('btn')[0]);
            clipboard.on('success', function() {
                self.notify('success', 'Copied to clipboard.');
            });
        });

        click(e) {
            e.preventDefault();
            e.stopPropagation();
        }
    </script>
</clipboard>