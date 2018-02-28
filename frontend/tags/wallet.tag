<wallet>
    <div class="container" if={ wallet }>
        <div class="row">
            <div class="col-sm-12">
                <div class="card mt-5">
                    <div class="card-header">User summary</div>
                    <ul class="list-group list-group-flush">
                        <li class="list-group-item">
                            <div class="row">
                                <div class="col-sm-3"><strong>Name:</strong></div>
                                <div class="col-sm-9">
                                    { wallet.name }
                                    <button class="btn btn-sm btn-outline-secondary ml-1" onclick={ logout }>Logout</button>
                                </div>
                            </div>
                        </li>
                        <li class="list-group-item">
                            <div class="row">
                                <div class="col-sm-3"><strong>Public key:</strong></div>
                                <div class="col-sm-9"><code>{ wallet.pub_key }</code></div>
                            </div>
                        </li>
                        <li class="list-group-item">
                            <div class="row">
                                <div class="col-sm-3"><strong>Balance:</strong></div>
                                <div class="col-sm-9">
                                    { numeral(wallet.balance).format('$0,0') }
                                    <button class="btn btn-sm btn-outline-success ml-1" data-toggle="modal" data-target="#addFundsModal">Add Funds</button>
                                    <button class="btn btn-sm btn-outline-primary ml-1" disabled={ wallet.balance == 0 } data-toggle="modal" data-target="#transferModal">Transfer Funds</button>
                                </div>
                            </div>
                        </li>
                        <li class="list-group-item">
                            <div class="row">
                                <div class="col-sm-3"><strong>Updated:</strong></div>
                                <div class="col-sm-9">{ moment(block.time / 1000000).fromNow() }</div>
                            </div>
                        </li>
                        <li class="list-group-item">
                            <div class="row">
                                <div class="col-sm-3"><strong>Block:</strong></div>
                                <div class="col-sm-9">{ block.height }</div>
                            </div>
                        </li>
                    </ul>
                </div>

                <div class="card mt-5">
                    <div class="card-header">Transactions</div>
                    <ul class="list-group list-group-flush">
                        <li class="list-group-item font-weight-bold">
                            <div class="row">
                                <div class="col-sm-4">Hash</div>
                                <div class="col-sm-5">Description</div>
                                <div class="col-sm-3">Status</div>
                            </div>
                        </li>
                        <li class="list-group-item" each={ transactions }>
                            <div class="row">
                                <div class="col-sm-4"><code>{ hash }</code></div>
                                <div class="col-sm-5" if={ message_id === 130 }>Wallet created</div>
                                <div class="col-sm-5" if={ message_id === 129 }>
                                    <strong>{ numeral(body.amount).format('$0,0') }</strong> funds added
                                </div>
                                <div class="col-sm-5" if={ message_id === 128 && body.from === publicKey }>
                                    <strong>{ numeral(body.amount).format('$0,0') }</strong> sent to <code>{ body.to }</code>
                                </div>
                                <div class="col-sm-5" if={ message_id === 128 && body.to === publicKey }>
                                    <strong>{ numeral(body.amount).format('$0,0') }</strong> received from <code>{ body.from }</code>
                                </div>
                                <div class="col-sm-3">
                                    <span if={ status } class="badge badge-success">executed</span>
                                    <span if={ !status } class="badge badge-danger">failed</span>
                                </div>
                            </div>
                        </li>
                    </ul>
                </div>
            </div>
        </div>
    </div>

    <div id="addFundsModal" class="modal" tabindex="-1" role="dialog">
        <div class="modal-dialog" role="document">
            <div class="modal-content">
                <form onsubmit={ addFunds }>
                    <div class="modal-header">
                        <h5 class="modal-title">Add Funds</h5>
                        <button type="button" class="close" data-dismiss="modal" aria-label="Close">
                            <span aria-hidden="true">&times;</span>
                        </button>
                    </div>

                    <div class="modal-body">
                        <div class="form-group">
                            <label class="d-block">Select amount to be added:</label>
                            <div class="form-check form-check-inline">
                                <input class="form-check-input" type="radio" name="sumToAdd" id="addFundsOne" value="10" checked>
                                <label class="form-check-label" for="addFundsOne">$10</label>
                            </div>
                            <div class="form-check form-check-inline">
                                <input class="form-check-input" type="radio" name="sumToAdd" id="addFundsTwo" value="50">
                                <label class="form-check-label" for="addFundsTwo">$50</label>
                            </div>
                            <div class="form-check form-check-inline">
                                <input class="form-check-input" type="radio" name="sumToAdd" id="addFundsThree" value="100">
                                <label class="form-check-label" for="addFundsThree">$100</label>
                            </div>
                        </div>
                    </div>

                    <div class="modal-footer">
                        <button type="button" class="btn btn-secondary" data-dismiss="modal">Close</button>
                        <button type="submit" class="btn btn-success">Add funds</button>
                    </div>
                </form>
            </div>
        </div>
    </div>

    <div id="transferModal" class="modal" tabindex="-1" role="dialog">
        <div class="modal-dialog" role="document">
            <div class="modal-content">
                <form onsubmit={ transfer }>
                    <div class="modal-header">
                        <h5 class="modal-title">Transfer Funds</h5>
                        <button type="button" class="close" data-dismiss="modal" aria-label="Close">
                            <span aria-hidden="true">&times;</span>
                        </button>
                    </div>

                    <div class="modal-body">
                        <div class="form-group">
                            <label>Receiver:</label>
                            <input type="text" class="form-control" placeholder="Enter public key" onkeyup={ editReceiver }>
                        </div>
                        <div class="form-group">
                            <label>Amount:</label>
                            <div class="input-group">
                                <div class="input-group-prepend">
                                    <div class="input-group-text">$</div>
                                </div>
                                <input type="number" class="form-control" placeholder="Enter amount" onkeyup={ editAmount }>
                            </div>
                        </div>
                    </div>

                    <div class="modal-footer">
                        <button type="button" class="btn btn-secondary" data-dismiss="modal">Close</button>
                        <button type="submit" class="btn btn-primary">Transfer</button>
                    </div>
                </form>
            </div>
        </div>
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
                        self.notify('error', error.toString());

                        self.logout();
                    });

                },
                error: function(jqXHR, textStatus, errorThrown) {
                    self.notify('error', errorThrown.toString());
                }
            });
        }).catch(function(error) {
            self.toggleLoading(false);

            self.notify('error', error.toString());

            route('/');
        });

        addFunds(e) {
            e.preventDefault();

            this.toggleLoading(true);

            this.auth.getUser().then(function(keyPair) {
                var TxIssue = Exonum.newMessage({
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

                var data = {
                    wallet: keyPair.publicKey,
                    amount: $('[name="sumToAdd"]:checked').val(),
                    seed: Exonum.randomUint64()
                };

                var signature = TxIssue.sign(keyPair.secretKey, data);

                $.ajax({
                    method: 'POST',
                    url: '/api/services/cryptocurrency/v1/wallets/transaction',
                    contentType: 'application/json; charset=utf-8',
                    data: JSON.stringify({
                        body: data,
                        network_id: self.NETWORK_ID,
                        protocol_version: self.PROTOCOL_VERSION,
                        service_id: self.SERVICE_ID,
                        message_id: self.TX_ISSUE_ID,
                        signature: signature
                    }),
                    success: function() {
                        self.toggleLoading(false);

                        self.notify('success', 'Add funds transaction has been sent');

                        $('#addFundsModal').modal('hide');
                    },
                    error: function(jqXHR, textStatus, errorThrown) {
                        throw errorThrown;
                    }
                });
            }).catch(function(error) {
                self.toggleLoading(false);

                self.notify('error', error.toString());
            });
        }

        editReceiver(e) {
            this.receiver = e.target.value;
        }

        editAmount(e) {
            if (e.target.value > 0 && e.target.value.toLowerCase().indexOf('e') === -1) {
                this.amount = e.target.value;
            } else {
                this.amount = 0;
            }
        }

        transfer(e) {
            e.preventDefault();

            this.toggleLoading(true);

            this.auth.getUser().then(function(keyPair) {
                var TxTransfer = Exonum.newMessage({
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

                var data = {
                    from: keyPair.publicKey,
                    to: self.receiver,
                    amount: self.amount.toString(),
                    seed: Exonum.randomUint64()
                };

                var signature = TxTransfer.sign(keyPair.secretKey, data);

                $.ajax({
                    method: 'POST',
                    url: '/api/services/cryptocurrency/v1/wallets/transaction',
                    contentType: 'application/json; charset=utf-8',
                    data: JSON.stringify({
                        body: data,
                        network_id: self.NETWORK_ID,
                        protocol_version: self.PROTOCOL_VERSION,
                        service_id: self.SERVICE_ID,
                        message_id: self.TX_TRANSFER_ID,
                        signature: signature
                    }),
                    success: function() {
                        self.toggleLoading(false);

                        self.notify('success', 'Transfer transaction has been sent');

                        $('#transferModal').modal('hide');
                    },
                    error: function(jqXHR, textStatus, errorThrown) {
                        throw errorThrown;
                    }
                });
            }).catch(function(error) {
                self.toggleLoading(false);

                self.notify('error', error.toString());
            });
        }

        logout(e) {
            if (typeof e !== 'undefined') {
                e.preventDefault();
            }

            self.auth.removeUser();

            route('/');
        }
    </script>
</wallet>
