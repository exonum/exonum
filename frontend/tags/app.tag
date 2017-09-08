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

            $.ajax({
                method: 'GET',
                url: 'configuration',
                success: function(response) {
                    var service = new CryptocurrencyService(response);

                    // global mixin with common functions and constants
                    riot.mixin({
                        service: service,

                        notify: function(type, text, timeout) {
                            noty({
                                layout: 'topCenter',
                                timeout: timeout || 5000,
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

                    // initialize routes
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
                },
                error: function(jqXHR, textStatus, errorThrown) {
                    throw errorThrown;
                }
            });
        });
    </script>
</app>