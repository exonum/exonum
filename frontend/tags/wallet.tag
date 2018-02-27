<wallet>
    <div class="panel-heading">
        <button class="btn btn-default pull-right page-nav" onclick={ refresh }>
            <i class="glyphicon glyphicon-refresh"></i>
            <span class="hidden-xs">Refresh</span>
        </button>
        <button class="btn btn-default pull-left page-nav" onclick={ logout }>
            <i class="glyphicon glyphicon-log-out"></i>
            <span class="hidden-xs">Logout</span>
        </button>
        <div class="panel-title page-title text-center">
            <div class="h4">Wallet</div>
        </div>
    </div>

    <div if={ wallet } class="panel-body">
        <summary wallet={ wallet } block={ block }/>

        <div class="form-group">
            <p class="text-center">Transfer your funds to another account:</p>
            <button class="btn btn-lg btn-block btn-primary" disabled={ wallet.balance == 0 } onclick={ transfer }>
                Transfer Funds
            </button>
        </div>

        <div class="form-group">
            <p class="text-center">Add more funds to your account:</p>
            <a href="#user/add-funds" class="btn btn-lg btn-block btn-success">Add Funds</a>
        </div>

        <history transactions={ transactions } public_key={ publicKey }/>
    </div>

    <script>
        var self = this;
        var attempts = 10;
        var TableKey = Exonum.newType({
            size: 4,
            fields: {
                service_id: {type: Exonum.Uint16, size: 2, from: 0, to: 2},
                table_index: {type: Exonum.Uint16, size: 2, from: 2, to: 4}
            }
        });
        var Wallet = Exonum.newType({
            size: 88,
            fields: {
                pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                name: {type: Exonum.String, size: 8, from: 32, to: 40},
                balance: {type: Exonum.Uint64, size: 8, from: 40, to: 48},
                history_len: {type: Exonum.Uint64, size: 8, from: 48, to: 56},
                history_hash: {type: Exonum.Hash, size: 32, from: 56, to: 88}
            }
        });
        var TransactionMetaData = Exonum.newType({
            size: 33,
            fields: {
                tx_hash: {type: Exonum.Hash, size: 32, from: 0, to: 32},
                execution_status: {type: Exonum.Bool, size: 1, from: 32, to: 33}
            }
        });

        function getTransactionPublicKey(transaction, id) {
            switch (id) {
                case self.TX_TRANSFER_ID:
                    return transaction.from;
                case self.TX_ISSUE_ID:
                    return transaction.wallet;
                case self.TX_WALLET_ID:
                    return transaction.pub_key;
                default:
                    throw new Error('Unknown transaction ID has been passed');
            }
        }

        function getTransaction(configuration, id) {
            switch (id) {
                case self.TX_WALLET_ID:
                    return Exonum.newMessage({
                        size: 40,
                        network_id: self.NETWORK_ID,
                        protocol_version: self.PROTOCOL_VERSION,
                        service_id: self.SERVICE_ID,
                        message_id: self.TX_WALLET_ID,
                        fields: {
                            pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            name: {type: Exonum.String, size: 8, from: 32, to: 40}
                        }
                    });
                case self.TX_ISSUE_ID:
                    return Exonum.newMessage({
                        size: 48,
                        network_id: self.NETWORK_ID,
                        protocol_version: self.PROTOCOL_VERSION,
                        service_id: self.SERVICE_ID,
                        message_id: self.TX_ISSUE_ID,
                        fields: {
                            wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            amount: {type: Exonum.Uint64, size: 8, from: 32, to: 40},
                            seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
                        }
                    });
                case self.TX_TRANSFER_ID:
                    return Exonum.newMessage({
                        size: 80,
                        network_id: self.NETWORK_ID,
                        protocol_version: self.PROTOCOL_VERSION,
                        service_id: self.SERVICE_ID,
                        message_id: self.TX_TRANSFER_ID,
                        fields: {
                            from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
                            amount: {type: Exonum.Int64, size: 8, from: 64, to: 72},
                            seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
                        }
                    });
                default:
                    throw new Error('Unknown transaction ID has been passed');
            }
        }

        function getPrecommitsMedianTime(precommits) {
            var values = [];
            for (var i = 0; i < precommits.length; i++) {
                var time = precommits[i].body.time;
                values.push(bigInt(time.secs).multiply(1000000000).plus(time.nanos));
            }
            values.sort(function(a, b) {
                return a.compare(b);
            });
            var half = Math.floor(values.length / 2);

            if (values.length % 2) {
                return values[half].toString();
            } else {
                return values[half - 1].plus(values[half]).divide(2).toString();
            }
        }

        function loadWallet(publicKey, validators) {
            return new Promise(function(resolve, reject) {
                $.ajax({
                    method: 'GET',
                    url: '/api/services/cryptocurrency/v1/wallets/info?pubkey=' + publicKey,
                    dataType: 'json',
                    success: function(data) {
                        if (!Exonum.verifyBlock(data.block_info, validators, self.NETWORK_ID)) {
                            return reject(new Error('Block can not be verified'));
                        }

                        var block = data.block_info.block;
                        block.time = getPrecommitsMedianTime(data.block_info.precommits);

                        var tableKey = TableKey.hash({
                            service_id: self.SERVICE_ID,
                            table_index: 0
                        });

                        // find root hash of table with wallets in the tree of all tables
                        var walletsHash = Exonum.merklePatriciaProof(block.state_hash, data.wallet.mpt_proof, tableKey);
                        if (walletsHash === null) {
                            if (attempts > 0) {
                                attempts--;
                                return resolve(loadWallet(validators));
                            } else {
                                return reject(new Error('Wallets table not found'));
                            }
                        }

                        // find wallet in the tree of all wallets
                        var wallet = Exonum.merklePatriciaProof(walletsHash, data.wallet.value, publicKey, Wallet);
                        if (wallet === null) {
                            if (attempts > 0) {
                                attempts--;
                                return resolve(loadWallet(validators));
                            } else {
                                return reject(new Error('Wallet not found'));
                            }
                        }

                        // get transactions
                        var transactionsMetaData = Exonum.merkleProof(
                            wallet.history_hash,
                            wallet.history_len,
                            data.wallet_history.mt_proof,
                            [0, wallet.history_len],
                            TransactionMetaData
                        );

                        if (data.wallet_history.values.length !== transactionsMetaData.length) {
                            // number of transactions in wallet history is not equal
                            // to number of transactions in array with transactions meta data
                            return reject(new Error('Transactions can not be verified'));
                        }

                        // validate each transaction
                        var transactions = [];
                        for (var i = 0; i < data.wallet_history.values.length; i++) {
                            var transaction = data.wallet_history.values[i];
                            var type = getTransaction(this.validators, transaction.message_id);
                            var publicKeyOfTransaction = getTransactionPublicKey(transaction.body, transaction.message_id);

                            type.signature = transaction.signature;
                            transaction.hash = type.hash(transaction.body);
                            transaction.status = transactionsMetaData[i].execution_status;

                            if (transaction.hash !== transactionsMetaData[i].tx_hash) {
                                // wrong transaction hash
                                return reject(new Error('Invalid transaction hash has been found'));
                            } else if (!type.verifySignature(transaction.signature, publicKeyOfTransaction, transaction.body)) {
                                // wrong transaction signature
                                return reject(new Error('Invalid transaction signature has been found'));
                            }

                            transactions.push(transaction);
                        }

                        resolve({
                            block: data.block_info.block,
                            wallet: wallet,
                            transactions: transactions
                        });
                    },
                    error: function(jqXHR, textStatus, errorThrown) {
                        reject(errorThrown);
                    }
                });
            });
        }

        this.toggleLoading(true);

        this.auth.getUser().then(function(keyPair) {
            $.ajax({
                method: 'GET',
                url: '/api/services/configuration/v1/configs/actual',
                dataType: 'json',
                success: function(response) {
                    var validators = response.config.validator_keys.map(function(validator) {
                        return validator.consensus_key;
                    });

                    loadWallet(keyPair.publicKey, validators).then(function(data) {
                        self.toggleLoading(false);

                        self.publicKey = keyPair.publicKey;
                        self.block = data.block;
                        self.wallet = data.wallet;
                        self.transactions = data.transactions;
                        self.update();
                    }).catch(function(error) {
                        self.notify('error', error);
                    });

                },
                error: function(jqXHR, textStatus, errorThrown) {
                    self.notify('error', errorThrown);
                }
            });
        }).catch(function(error) {
            self.toggleLoading(true);

            self.notify('error', error);
        });

        transfer(e) {
            e.preventDefault();

            route('/user/transfer');
        }

        refresh(e) {
            e.preventDefault();

            window.location.reload();
        }

        logout(e) {
            e.preventDefault();

            self.auth.removeUser();

            route('/');
        }
    </script>
</wallet>
