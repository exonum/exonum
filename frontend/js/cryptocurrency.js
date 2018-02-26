/* eslint-env jquery */
/* global Exonum, bigInt, pwbox */

/**
 * Business logic
 */

(function (window) {

    var NETWORK_ID = 0;
    var PROTOCOL_VERSION = 0;
    var SERVICE_ID = 128;
    var TX_TRANSFER_FUNDS_ID = 128;
    var TX_ADD_FUNDS_ID = 129;
    var TX_CREATE_WALLET_ID = 130;
    var TransferTransaction = {
        size: 80,
        message_id: TX_TRANSFER_FUNDS_ID,
        fields: {
            from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
            amount: {type: Exonum.Int64, size: 8, from: 64, to: 72},
            seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
        }
    };
    var AddFundsTransaction = {
        size: 48,
        message_id: TX_ADD_FUNDS_ID,
        fields: {
            wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            amount: {type: Exonum.Int64, size: 8, from: 32, to: 40},
            seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
        }
    };
    var CreateWalletTransaction = {
        size: 168,
        message_id: TX_CREATE_WALLET_ID,
        fields: {
            pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            login: {type: Exonum.String, size: 8, from: 32, to: 40},
            key_box: {type: Exonum.FixedBuffer, size: 128, from: 40, to: 168}
        }
    };
    var Wallet = Exonum.newType({
        size: 88,
        fields: {
            pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            login: {type: Exonum.String, size: 8, from: 32, to: 40},
            balance: {type: Exonum.Uint64, size: 8, from: 40, to: 48},
            history_len: {type: Exonum.Uint64, size: 8, from: 48, to: 56},
            history_hash: {type: Exonum.Hash, size: 32, from: 56, to: 88}
        }
    });
    var TableKey = Exonum.newType({
        size: 4,
        fields: {
            service_id: {type: Exonum.Uint16, size: 2, from: 0, to: 2},
            table_index: {type: Exonum.Uint16, size: 2, from: 2, to: 4}
        }
    });
    var TransactionMetaData = Exonum.newType({
        size: 33,
        fields: {
            tx_hash: {type: Exonum.Hash, size: 32, from: 0, to: 32},
            execution_status: {type: Exonum.Bool, size: 1, from: 32, to: 33}
        }
    });

    function getData(url, callback) {
        $.ajax({
            method: 'GET',
            url: url,
            dataType: 'json',
            success: function(response) {
                if (typeof response === 'object') {
                    callback(null, response);
                } else {
                    callback(new TypeError('Unknown format of server response'));
                }
            },
            error: function(jqXHR, textStatus, errorThrown) {
                callback(errorThrown);
            }
        });
    }

    function createTransactionType(spec, configuration) {
        var txConfiguration = {
            network_id: NETWORK_ID,
            protocol_version: PROTOCOL_VERSION,
            service_id: SERVICE_ID
        };
        return Exonum.newMessage(Object.assign({}, txConfiguration, spec));
    }

    function getTransaction(configuration, id) {
        switch (id) {
            // transfer funds
            case TX_TRANSFER_FUNDS_ID:
                return createTransactionType(TransferTransaction, configuration);
            // add funds
            case TX_ADD_FUNDS_ID:
                return createTransactionType(AddFundsTransaction, configuration);
            // create wallet
            case TX_CREATE_WALLET_ID:
                return createTransactionType(CreateWalletTransaction, configuration);
            default:
                throw new Error('Unknown transaction ID has been passed.');
        }
    }

    function getPublicKeyOfTransaction(transaction, id) {
        switch (id) {
            case TX_TRANSFER_FUNDS_ID:
                return transaction.from;
            case TX_ADD_FUNDS_ID:
                return transaction.wallet;
            case TX_CREATE_WALLET_ID:
                return transaction.pub_key;
            default:
                throw new Error('Unknown transaction ID has been passed.');
        }
    }

    function submitTransaction(id, data, publicKey, secretKey, callback) {
        var self = this;
        var type = getTransaction(this.validators, id);

        type.signature = type.sign(secretKey, data);

        var hash = type.hash(data);

        function loop() {
            loadWallet.call(self, publicKey, function(error, block, wallet, transactions) {
                if (error) {
                    if (typeof callback === 'function') {
                        callback(error);
                    }
                    return;
                }

                if (Array.isArray(transactions)) {
                    for (var i = 0; i < transactions.length; i++) {
                        if (transactions[i].hash === hash) {
                            if (typeof callback === 'function') {
                                callback(null, block, wallet, transactions);
                            }
                            return;
                        }
                    }
                }

                setTimeout(loop, 1000);
            });
        }

        $.ajax({
            method: 'POST',
            url: 'api/services/cryptocurrency/v1/wallets/transaction',
            contentType: 'application/json; charset=utf-8',
            data: JSON.stringify({
                body: data,
                network_id: NETWORK_ID,
                protocol_version: PROTOCOL_VERSION,
                service_id: SERVICE_ID,
                message_id: type.message_id,
                signature: type.signature
            }),
            success: function(response) {
                if (typeof response === 'object') {
                    loop();
                } else {
                    callback(new TypeError('Unknown format of server response'));
                }
            },
            error: function(jqXHR, textStatus, errorThrown) {
                callback(errorThrown);
            }
        });
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

    function parseWalletProof(publicKey, data) {
        if (!Exonum.verifyBlock(data.block_info, this.validators, NETWORK_ID)) {
            return;
        }

        var block = data.block_info.block;
        block.time = getPrecommitsMedianTime(data.block_info.precommits);

        // find root hash of table with wallets in the tree of all tables
        var tableKeyData = {
            service_id: SERVICE_ID,
            table_index: 0
        };
        var tableKey = TableKey.hash(tableKeyData);
        var walletsHash = Exonum.merklePatriciaProof(block.state_hash, data.wallet.mpt_proof, tableKey);
        if (walletsHash === null) {
            return;
        }

        // find wallet in the tree of all wallets
        var wallet = Exonum.merklePatriciaProof(walletsHash, data.wallet.value, publicKey, Wallet);
        if (wallet === null) {
            // wallet is not found
            return [data.block_info.block];
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
            return;
        }

        // validate each transaction
        var transactions = [];
        for (var i = 0; i < data.wallet_history.values.length; i++) {
            var transaction = data.wallet_history.values[i];
            var type = getTransaction(this.validators, transaction.message_id);
            var publicKeyOfTransaction = getPublicKeyOfTransaction(transaction.body, transaction.message_id);

            type.signature = transaction.signature;
            transaction.hash = type.hash(transaction.body);
            transaction.status = transactionsMetaData[i].execution_status;

            if (transaction.hash !== transactionsMetaData[i].tx_hash) {
                // wrong transaction hash
                return;
            } else if (!type.verifySignature(transaction.signature, publicKeyOfTransaction, transaction.body)) {
                // wrong transaction signature
                return;
            }

            transactions.push(transaction);
        }

        return [data.block_info.block, wallet, transactions];
    }

    function loadWallet(publicKey, callback) {
        var self = this;
        var url = 'api/services/cryptocurrency/v1/wallets/info?pubkey=' + publicKey;

        getData(url, function(error, response) {
            if (error) {
                callback(error);
                return;
            }

            try {
                var wallet = parseWalletProof.call(self, publicKey, response);
            } catch (e) {
                callback(e);
                return;
            }

            callback.apply(undefined, [null].concat(wallet));
        });
    }

    function CryptocurrencyService(validators) {
        this.validators = validators;
    }

    CryptocurrencyService.prototype.getWallet = function(publicKey, callback) {
        loadWallet.call(this, publicKey, callback);
    };

    CryptocurrencyService.prototype.createWallet = function(login, password, callback) {
        var self = this;
        var pair = Exonum.keyPair();
        var message = Exonum.hexadecimalToUint8Array(pair.secretKey);

        pwbox(message, password, function(err, box) {
            if (err) {
                console.error(err);
                return;
            }

            var data = {
                login: login,
                pub_key: pair.publicKey,
                key_box: Exonum.uint8ArrayToHexadecimal(box)
            };

            submitTransaction.call(self, TX_CREATE_WALLET_ID, data, pair.publicKey, pair.secretKey, callback);
        });
    };

    CryptocurrencyService.prototype.login = function(login, password, callback) {
        var url = 'api/services/cryptocurrency/v1/wallets/find/' + encodeURIComponent(login);

        getData(url, function(err, response) {
            if (err) {
                callback(err);
                return;
            }

            pwbox.open(Exonum.hexadecimalToUint8Array(response.key_box), password, function(err, message) {
                if (err) {
                    console.error(err);
                    callback(err);
                    return;
                }

                callback(null, response.pub_key, Exonum.uint8ArrayToHexadecimal(message));
            });
        });
    };

    CryptocurrencyService.prototype.addFunds = function(amount, publicKey, secretKey, callback) {
        var seed = Exonum.randomUint64();
        var data = {
            wallet: publicKey,
            amount: amount,
            seed: seed
        };

        submitTransaction.call(this, TX_ADD_FUNDS_ID, data, publicKey, secretKey, callback);
    };

    CryptocurrencyService.prototype.transfer = function(amount, from, to, secretKey, callback) {
        var seed = Exonum.randomUint64();
        var data = {
            from: from,
            to: to,
            amount: amount,
            seed: seed
        };

        submitTransaction.call(this, TX_TRANSFER_FUNDS_ID, data, from, secretKey, callback);
    };

    CryptocurrencyService.prototype.getTransactionDescription = function(id) {
        switch (id) {
            case TX_TRANSFER_FUNDS_ID:
                return 'Transfer';
            case TX_ADD_FUNDS_ID:
                return 'Add Funds';
            case TX_CREATE_WALLET_ID:
                return 'Create Wallet';
            default:
                return 'Unknown';
        }
    };

    CryptocurrencyService.prototype.getBlocks = function(height, count, callback) {
        var suffix = '';

        if (!isNaN(height)) {
            suffix += '&latest=' + height;
        }

        var url = 'api/explorer/v1/blocks?count=' + count + suffix;

        getData(url, callback);
    };

    CryptocurrencyService.prototype.getBlock = function(height, callback) {
        var url = 'api/explorer/v1/blocks/' + height;

        getData(url, callback);
    };

    CryptocurrencyService.prototype.getTransaction = function(hash, callback) {
        var url = 'api/system/v1/transactions/' + hash;

        getData(url, callback);
    };

    window.CryptocurrencyService = CryptocurrencyService;

})(window);
