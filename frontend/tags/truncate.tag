<truncate>
    <span class="monospace collapsed" if={ collapsed } onclick={ expand }>{ truncated }</span>
    <span class="monospace expanded" if={ !collapsed }>{ opts.val }</span>

    <script>
        this.collapsed = true;
        this.truncated = this.opts.val.substring(0, this.opts.digits || 8) + '…';

        expand(e) {
            e.preventDefault();
            if (this.collapsed) {
                this.collapsed = false;
                this.update();
            }
        }
    </script>
</truncate>