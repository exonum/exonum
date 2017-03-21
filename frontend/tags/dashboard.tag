<dashboard>
    <div class="panel-heading">
        <div class="panel-title page-title text-center">
            <div class="h4">Cryptocurrency demo <span class="hidden-xs">application</span></div>
        </div>
    </div>
    <div class="panel-body text-center">
        <div class="form-group">
            <p>Create new wallet:</p>
            <a href="#register" class="btn btn-lg btn-block btn-success">Register</a>
        </div>

        <div class="form-group" if={ users }>
            <p>Login into existed wallet:</p>
            <!--<a href="#login" class="btn btn-lg btn-block btn-primary">Login</a>-->
            <a href="#user/{ publicKey }" class="btn btn-lg btn-block btn-primary" each={ users }>{name}</a>
        </div>

        <div class="form-group">
            <p>Explore blockchain:</p>
            <a href="#blockchain" class="btn btn-lg btn-block btn-default">Blockchain Explorer</a>
        </div>
    </div>

    <script>
        this.users = this.localStorage.getUsers();
    </script>
</dashboard>