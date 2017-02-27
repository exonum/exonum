<register>
    <form class="form-horizontal" onsubmit="{ register }">
        <legend class="text-center">Create wallet</legend>
        <div class="form-group">
            <div class="col-sm-4 control-label">Your name:</div>
            <div class="col-sm-8">
                <input type="text" class="form-control" onkeyup="{ edit }">
            </div>
        </div>
        <div class="form-group">
            <div class="col-sm-offset-4 col-sm-8">
                <button type="submit" class="btn btn-lg btn-primary" disabled={ !text }>Create wallet</button>
                <a href="/#/" class="btn btn-lg btn-default">Back</a>
            </div>
        </div>
    </form>

    <script>
        this.on('mount', function() {
            this.opts.title.trigger('change', 'Register');
        });

        edit(e) {
            this.text = e.target.value;
        }

        register(e) {
            if (this.text) {
                // TODO do ajax request, redirect or show error
            }
            e.preventDefault()
        }
    </script>
</register>