<welcome>
    <div class="text-center">
        <p class="lead">Welcome in cryptocurrency demo application.</p>
        <div if={ !users } class="alert alert-warning">
            <i class="glyphicon glyphicon-warning-sign"></i> You haven't any wallet yet.
        </div>
        <p each={ users }><a href="/#user/{ publicKey }" class="btn btn-lg btn-block btn-primary">Log in as {name}</a></p>
        <p><a href="/#register" class="btn btn-lg btn-block btn-default">Create wallet</a></p>
        <a href="/#blockchain" class="btn btn-lg btn-link">View blockchain</a>
    </div>

    <script>
        this.title = 'Login';
        this.users = this.localStorage.getUsers();
    </script>
</welcome>