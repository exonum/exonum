<app>
    <div id="content"></div>

    <footer class="pb-4 hr">
        <hr class="mt-5 mb-5">
        <div class="container">
            <div class="row">
                <div class="col-sm-12">
                    <img src="images/exonum.png" width="41" height="36" class="float-left mt-sm-1 mr-3" alt="">
                    <ul class="list-unstyled">
                        <li>Sources on <a href="https://github.com/exonum/cryptocurrency-advanced" target="_blank">GitHub</a></li>
                        <li><a href="https://exonum.com/doc/" target="_blank">Exonum docs</a></li>
                    </ul>
                </div>
            </div>
        </div>
    </footer>

    <div class="loader" if={ loading }></div>

    <style>
        .loader {
            height: 100%;
            left: 0;
            position: fixed;
            top: 0;
            width: 100%;
            z-index: 10000;
        }

        .loader:before {
            background-color: rgba(255, 255, 255, .9);
            content: '';
            display: block;
            height: 100%;
            left: 0;
            position: absolute;
            top: 0;
            width: 100%;
        }

        .loader:after {
            background-image: url(images/loader.gif);
            background-size: 100%;
            content: '';
            display: block;
            height: 60px;
            left: 50%;
            margin: -30px 0 0 -30px;
            position: absolute;
            top: 50%;
            width: 60px;
        }
    </style>

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

                notify: function(type, text) {
                    new Noty({
                        theme: 'bootstrap-v4',
                        timeout: 5000,
                        type: type || 'information',
                        text: text,
                        killer: true,
                        progressBar: false
                    }).show();
                },

                toggleLoading: function(state) {
                    self.loading = state;
                    self.update();
                },

                validateHash: function(hash, bytes) {
                    bytes = bytes || 32;

                    if (typeof hash !== 'string') {
                        return false;
                    } else if (hash.length !== bytes * 2) {
                        // 'hexadecimal string is of wrong length
                        return false;
                    }

                    for (var i = 0; i < hash.length; i++) {
                        if (isNaN(parseInt(hash[i], 16))) {
                            // invalid symbol in hexadecimal string
                            return false;
                        }
                    }

                    return true;
                }
            });

            // initialize routes
            route('/', function() {
                riot.mount('#content', 'auth');
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
