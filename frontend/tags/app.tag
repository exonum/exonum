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
                url: '/api/services/configuration/v1/configs/actual',
                dataType: 'json',
                success: function(response) {
                    var validators = response.config.validator_keys.map(function(validator) {
                        return validator.consensus_key;
                    });
                    var service = new CryptocurrencyService(validators);

                    // global mixin with common functions and constants
                    riot.mixin({
                        core: Exonum,
                        service: service,

                        notify: function(type, text, timeout) {
                            noty({
                                layout: 'topCenter',
                                timeout: typeof timeout === 'undefined' ? 5000 : timeout,
                                type: type || 'information',
                                text: text
                            });
                        },


                        auth: {
                            getUser: function() {
                                return JSON.parse(window.localStorage.getItem('user'));
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
                },
                error: function(jqXHR, textStatus, errorThrown) {
                    throw errorThrown;
                }
            });
        });
    </script>
</app>
