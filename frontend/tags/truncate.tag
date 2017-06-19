<truncate>
    <span class="monospace collapsed" if={ collapsed } onclick={ expand }>{ truncated }</span>
    <div class="monospace expanded" if={ !collapsed }>
        <div class="truncate-item" each={ line in lines }>{ line }</div>
        <clipboard val={ opts.val }></clipboard>
    </div>

    <script>
        this.collapsed = true;
        this.digits = this.opts.digits || 8;
        this.truncated = this.opts.val.substring(0, this.digits) + '…';
        this.lines = [];

        for (var i = 0, len = opts.val.length; i < len; i += this.digits) {
            this.lines.push(this.opts.val.substring(i, i + this.digits));
        }

        expand(e) {
            if (this.collapsed) {
                e.preventDefault();
                e.stopPropagation();

                this.collapsed = false;
                this.update();
            }
        }
    </script>
</truncate>