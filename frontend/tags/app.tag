<app>
    <div class="container">
        <div class="row">
            <div class="col-sm-6 col-sm-offset-3">
                <div id="content" class="panel panel-default"></div>
                <p class="text-center text-muted">Find out more on <a href="http://exonum.com/" target="_blank">exonum.com</a></p>
            </div>
        </div>
    </div>

    <div class="loader" if={ loading }></div>

    <script>
        var self = this;

        // init app routes
        this.on('mount', function() {
            // global mixin with common functions and constants
            riot.mixin({
                NETWORK_ID: 0,
                PROTOCOL_VERSION: 0,
                SERVICE_ID: 128,
                TX_WALLET_ID: 130,
                TX_ISSUE_ID: 129,
                TX_TRANSFER_ID: 128,

                notify: function(type, text) {
                    noty({
                        layout: 'topCenter',
                        timeout: 5000,
                        type: type || 'information',
                        text: text
                    });
                },

                auth: {
                    getUser: function() {
                        return new Promise(function(resolve, reject) {
                            var keyPair = JSON.parse(window.localStorage.getItem('user'));

                            if (keyPair === null) {
                                return reject(new Error('User not found in local storage'));
                            }

                            resolve(keyPair);
                        })
                    },
                    setUser: function(user) {
                        window.localStorage.setItem('user', JSON.stringify(user));
                    },
                    removeUser: function() {
                        window.localStorage.removeItem('user');
                    }
                },

                toggleLoading: function(state) {
                    self.loading = state;
                    self.update();
                }
            });

            // initialize routes
            route('/', function() {
                riot.mount('#content', 'dashboard');
            });

            route('/login', function() {
                riot.mount('#content', 'login');
            });

            route('/register', function() {
                riot.mount('#content', 'register');
            });

            route('/user', function() {
                riot.mount('#content', 'wallet');
            });

            route('/user/transfer', function() {
                riot.mount('#content', 'transfer');
            });

            route('/user/add-funds', function() {
                riot.mount('#content', 'add-funds');
            });

            route('/blockchain', function() {
                riot.mount('#content', 'blockchain');
            });

            route('/blockchain/block/*', function(height) {
                riot.mount('#content', 'block', {height: height});
            });

            route('/blockchain/transaction/*', function(hash) {
                riot.mount('#content', 'transaction', {hash: hash});
            });

            route.start(true);
        });
    </script>
</app>
