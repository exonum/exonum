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
        var service = new CryptocurrencyService();

        // global mixin with common functions and constants
        riot.mixin({

            service: service,

            // TODO revert later
//            auth: {
//                setUser: function(user) {
//                    window.localStorage.setItem('user', JSON.stringify(user));
//                },
//
//                getUser: function() {
//                    return JSON.parse(window.localStorage.getItem('user')); // TODO rework?
//                },
//
//                logout: function() {
//                    window.localStorage.removeItem('user'); // TODO rework?
//                }
//            },

            notify: function(type, text) {
                noty({
                    layout: 'topCenter',
                    timeout: 5000,
                    type: type || 'information',
                    text: text
                });
            },

            core: Exonum,
            storage: {
                getUsers: function() {
                    return JSON.parse(window.localStorage.getItem('cc_users')) || [];
                },
                addUser: function(user) {
                    var users = JSON.parse(window.localStorage.getItem('cc_users')) || [];
                    users.push(user);
                    window.localStorage.setItem('cc_users', JSON.stringify(users));
                },
                getUser: function(publicKey) {
                    var users = JSON.parse(window.localStorage.getItem('cc_users')) || [];
                    for (var i = 0; i < users.length; i++) {
                        if (users[i].publicKey === publicKey) {
                            return users[i];
                        }
                    }
                }
            },

            toggleLoading: function(state) {
                self.loading = state;
                self.update();
            }
        });

        // init app routes
        this.on('mount', function() {

            route('/', function() {
                riot.mount('#content', 'welcome');
            });

            route('/dashboard', function() {
                riot.mount('#content', 'dashboard');
            });

            route('/login', function() {
                riot.mount('#content', 'login');
            });

            route('/register', function() {
                riot.mount('#content', 'register');
            });

            // TODO revert later
//            route('/user', function() {
//                riot.mount('#content', 'wallet');
//            });
//
//            route('/user/transfer', function() {
//                riot.mount('#content', 'transfer');
//            });
//
//            route('/user/add-funds', function() {
//                riot.mount('#content', 'add-funds');
//            });

            route('/user/*', function(publicKey) {
                riot.mount('#content', 'wallet', {publicKey: publicKey});
            });

            route('/user/*/transfer', function(publicKey) {
                riot.mount('#content', 'transfer', {publicKey: publicKey});
            });

            route('/user/*/add-funds', function(publicKey) {
                riot.mount('#content', 'add-funds', {publicKey: publicKey});
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