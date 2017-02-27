<app>
    <div class="container">
        <div class="row">
            <div class="col-sm-6 col-sm-offset-3">
                <div class="panel panel-default">
                    <div class="panel-heading">
                        <div class="panel-title">
                            <div id="title" class="h4">{ title }</div>
                        </div>
                    </div>
                    <div id="content" class="panel-body"></div>
                </div>
            </div>
        </div>
    </div>

    <script>

        var self = this;

        this.on('mount', function() {

            var title = riot.observable();

            title.on('change', function(value) {
                self.title = value;
                self.update();
            });

            route('/', function() {
                var register = riot.mount('#content', 'welcome', {title: title});
            });

            route('/register', function() {
                var register = riot.mount('#content', 'register', {title: title});
            });

            route('/user/*', function(publicKey) {
                var wallet = riot.mount('#content', 'wallet', {title: title});
            });

            route('/user/*/transfer', function(publicKey) {
                var blockchain = riot.mount('#content', 'transfer', {title: title});
            });

            route('/user/*/add-funds', function(publicKey) {
                var blockchain = riot.mount('#content', 'add-funds', {title: title});
            });

            route('/blockchain', function() {
                var blockchain = riot.mount('#content', 'blockchain', {title: title});
            });

            route('/blockchain/*', function(hash) {
                var block = riot.mount('#content', 'block', {title: title});
            });

            route('/blockchain/transaction/*', function(hash) {
                var block = riot.mount('#content', 'transaction', {title: title});
            });

            route.start(true);
        });

    </script>
</app>