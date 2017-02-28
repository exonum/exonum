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

            var Common = {
                api: {
                    baseUrl: 'http://exonum.com/backends/currency/api/v1'
                },

                init: function() {
                    this.on('mount', function() {
                        self.title = this.title;
                        self.update();
                    });
                }
            };

            riot.mixin(Common);

            route('/', function() {
                var register = riot.mount('#content', 'welcome');
            });

            route('/register', function() {
                var register = riot.mount('#content', 'register');
            });

            route('/user/*', function(publicKey) {
                var wallet = riot.mount('#content', 'wallet');
            });

            route('/user/*/transfer', function(publicKey) {
                var blockchain = riot.mount('#content', 'transfer');
            });

            route('/user/*/add-funds', function(publicKey) {
                var blockchain = riot.mount('#content', 'add-funds');
            });

            route('/blockchain', function() {
                var blockchain = riot.mount('#content', 'blockchain');
            });

            route('/blockchain/*', function(hash) {
                var block = riot.mount('#content', 'block');
            });

            route('/blockchain/transaction/*', function(hash) {
                var block = riot.mount('#content', 'transaction');
            });

            route.start(true);
        });

    </script>
</app>