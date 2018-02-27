<login>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#">
            <i class="glyphicon glyphicon-arrow-left"></i>
            <span class="hidden-xs">Back</span>
        </a>
        <div class="panel-title page-title text-center">
            <div class="h4">Login</div>
        </div>
    </div>

    <div class="panel-body">
        <form onsubmit={ submit }>
            <div class="form-group">
                <label class="control-label">Public key:</label>
                <input type="text" class="form-control" onkeyup={ editPublicKey }>
            </div>
            <div class="form-group">
                <label class="control-label">Secret key:</label>
                <input type="password" class="form-control" onkeyup={ editSecretKey }>
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-block btn-primary" disabled={ !publicKey || !secretKey }>Login</button>
            </div>
        </form>
    </div>

    <script>
        editPublicKey(e) {
            this.publicKey = e.target.value;
        }

        editSecretKey(e) {
            this.secretKey = e.target.value;
        }

        submit(e) {
            e.preventDefault();

            this.toggleLoading(true);

            this.auth.setUser({
                publicKey: this.publicKey,
                secretKey: this.secretKey
            });

            route('/user');
        }
    </script>
</login>
