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

            // shared observable to manage app title
            var titleObservable = riot.observable();

            titleObservable.on('change', function(value) {
                self.title = value;
                self.update();
            });

            // global mixin with common functions and constants
            var Common = {
                api: {
                    baseUrl: 'http://exonum.com/backends/currency/api/v1'
                },

                init: function() {
                    this.on('mount', function() {
                        // add title if it is predefined in component
                        if (this.title) {
                            self.title = this.title;
                            self.update();
                        }
                    });
                }
            };

            riot.mixin(Common);

            route('/', function() {
                riot.mount('#content', 'welcome');
            });

            route('/register', function() {
                riot.mount('#content', 'register');
            });

            route('/user/*', function(publicKey) {
                riot.mount('#content', 'wallet', {publicKey: publicKey, titleObservable: titleObservable});
            });

            route('/user/*/transfer', function(publicKey) {
                riot.mount('#content', 'transfer');
            });

            route('/user/*/add-funds', function(publicKey) {
                riot.mount('#content', 'add-funds');
            });

            route('/blockchain', function() {
                riot.mount('#content', 'blockchain');
            });

            route('/blockchain/block/*', function(height) {
                riot.mount('#content', 'block', {height: height});
            });

            route('/blockchain/transaction/*', function(hash) {
                riot.mount('#content', 'transaction', {hash: hash, titleObservable: titleObservable});
            });

            route.start(true);
        });

    </script>
</app>