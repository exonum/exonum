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
        var service = new CryptocurrencyService({
            id: 128,
            validators: [
                '7e2b6889b2e8b60e0e8d71be55b9cbf6aaa9bf397ef7b1d6b8564d862b120bea',
                '2f1e58c0752503e3b66a5f68d97ab44cac196c75608b53682c3da1f824f9391f',
                '8ce8ba0974e10d45d89b48a409015ebfe15a4aa9f9410951b266764b91c9d535',
                '11110c9c4b06d7cc0df9311aae089771b04b696a8eaa105ba39a186bcceed0c2'
            ],
            baseUrl: 'http://exonum.com/backends/currency/api/v1'
        });

        // global mixin with common functions and constants
        riot.mixin({
            core: Exonum,
            service: service,

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

            notify: function(type, text) {
                noty({
                    layout: 'topCenter',
                    timeout: 5000,
                    type: type || 'information',
                    text: text
                });
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

            route('/user/*', function(publicKey) {
                riot.mount('#content', 'wallet', {publicKey: publicKey});
            });

            route('/user/*/transfer', function(publicKey) {
                riot.mount('#content', 'transfer', {publicKey: publicKey});
            });

            route('/user/*/add-funds', function(publicKey) {
                riot.mount('#content', 'add-funds', {publicKey: publicKey});
            });

            route('/blockchain', function(height) {
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