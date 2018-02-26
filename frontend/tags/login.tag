<login>
    <div class="panel-heading">
        <a class="btn btn-default pull-left page-nav" href="#dashboard">
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
                <label class="control-label">Login:</label>
                <input type="text" class="form-control" onkeyup="{ editLogin }">
            </div>
            <div class="form-group">
                <label class="control-label">Password:</label>
                <input type="password" class="form-control" onkeyup="{ editPassword }">
            </div>
            <div class="form-group">
                <button type="submit" class="btn btn-lg btn-block btn-primary" disabled={ !login || !password }>Login</button>
            </div>
        </form>
    </div>

    <script>
        var self = this;

        editLogin(e) {
            this.login = e.target.value;
        }

        editPassword(e) {
            this.password = e.target.value;
        }

        submit(e) {
            e.preventDefault();

            self.toggleLoading(true);
            self.service.login(self.login, self.password, function(error, publicKey, secretKey) {
                self.toggleLoading(false);

                if (error) {
                    self.notify('error', error.message);
                    return;
                }

                self.auth.setUser({
                    publicKey: publicKey,
                    secretKey: secretKey
                });

                route('/user');
            }, function() {
                self.toggleLoading(false);
                self.notify('error', 'Wrong login or password has been passed.');
            });
        }
    </script>
</login>
