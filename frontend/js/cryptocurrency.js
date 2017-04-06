/**
 * Business logic
 */
function CryptocurrencyService(params) {

    this.id = params.id;

    this.validators = params.validators;

    this.baseUrl = params.baseUrl;

    this.Wallet = Exonum.newType({
        size: 88,
        fields: {
            pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            login: {type: Exonum.String, size: 8, from: 32, to: 40},
            balance: {type: Exonum.Uint64, size: 8, from: 40, to: 48},
            history_len: {type: Exonum.Uint64, size: 8, from: 48, to: 56},
            history_hash: {type: Exonum.Hash, size: 32, from: 56, to: 88}
        }
    });

    this.AddFundsTransactionParams = {
        size: 48,
        service_id: params.id,
        message_id: 129,
        fields: {
            wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            amount: {type: Exonum.Int64, size: 8, from: 32, to: 40},
            seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
        }
    };

    this.CreateWalletTransactionParams = {
        size: 144,
        service_id: params.id,
        message_id: 130,
        fields: {
            pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            login: {type: Exonum.String, size: 8, from: 32, to: 40},
            sec_key_enc: {type: Exonum.String, size: 80, from: 40, to: 120},
            nonce: {type: Exonum.Nonce, size: 24, from: 120, to: 144}
        }
    };

    this.TransferTransactionParams = {
        size: 80,
        service_id: params.id,
        message_id: 128,
        fields: {
            from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
            amount: {type: Exonum.Int64, size: 8, from: 64, to: 72},
            seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
        }
    };

    this.getTransactionTypeParams = function(id) {
        switch (id) {
            case 128:
                return new Exonum.newMessage(this.TransferTransactionParams);
                break;
            case 129:
                return new Exonum.newMessage(this.AddFundsTransactionParams);
                break;
            case 130:
                return new Exonum.newMessage(this.CreateWalletTransactionParams);
                break;
        }
    };

    this.submitTransaction = function(typeParams, data, publicKey, secretKey, callback) {
        var self = this;
        var type = new Exonum.newMessage(typeParams);

        type.signature = type.sign(data, secretKey);

        var hash = type.hash(data);

        function loop() {
            self.getWallet(publicKey, function(block, wallet, transactions) {
                if (Array.isArray(transactions)) {
                    for (var i = 0; i < transactions.length; i++) {
                        if (transactions[i].hash === hash) {
                            callback();
                            return;
                        }
                    }
                }

                setTimeout(loop, 1000);
            });
        }

        $.ajax({
            method: 'POST',
            url: this.baseUrl + '/wallets/transaction',
            contentType: 'application/json',
            data: JSON.stringify({
                body: data,
                message_id: type.message_id,
                service_id: type.service_id,
                signature: type.signature
            }),
            success: function(response, textStatus, jqXHR) {
                loop();
            },
            error: function(jqXHR, textStatus, errorThrown) {
                console.error(textStatus);
            }
        });
    };

    this.validateWallet = function(publicKey, data) {
        function getPublicKeyOfTransaction(id, transaction) {
            switch (id) {
                case 128:
                    return transaction.from;
                    break;
                case 129:
                    return transaction.wallet;
                    break;
                case 130:
                    return transaction.pub_key;
                    break;
            }
        }

        // validate block
        if (!Exonum.verifyBlock(data.block_info, params.validators)) {
            return;
        }

        // find root hash of table with wallets in the tree of all tables
        var TableKey = Exonum.newType({
            size: 4,
            fields: {
                service_id: {type: Exonum.Uint16, size: 2, from: 0, to: 2},
                table_index: {type: Exonum.Uint16, size: 2, from: 2, to: 4}
            }
        });
        var tableKeyData = {
            service_id: params.id,
            table_index: 0
        };
        var tableKey = TableKey.hash(tableKeyData);
        var walletsHash = Exonum.merklePatriciaProof(data.block_info.block.state_hash, data.wallet.mpt_proof, tableKey);
        if (walletsHash === null) {
            return;
        }

        // find wallet in the tree of all wallets
        var wallet = Exonum.merklePatriciaProof(walletsHash, data.wallet.value, publicKey, this.Wallet);
        if (wallet === null) {
            return;
        }

        // find hashes of all transactions
        var Transaction = Exonum.newType({
            size: 33,
            fields: {
                tx_hash: {type: Exonum.Hash, size: 32, from: 0, to: 32},
                execution_status: {type: Exonum.Bool, size: 1, from: 32, to: 33}
            }
        });
        var hashes = Exonum.merkleProof(wallet.history_hash, wallet.history_len, data.wallet_history.mt_proof, [0, wallet.history_len], Transaction);

        if (data.wallet_history.values.length !== hashes.length) {
            console.error('Number of transaction hashes is not equal to transactions number.');
            return;
        }

        // validate each transaction
        var transactions = [];
        for (var i = 0; i < data.wallet_history.values.length; i++) {
            var transaction = data.wallet_history.values[i];
            var type = this.getTransactionTypeParams(transaction.message_id);
            var publicKeyOfTransaction = getPublicKeyOfTransaction(transaction.message_id, transaction.body);
            
            type.signature = transaction.signature;
            transaction.hash = type.hash(transaction.body);
            transaction.status = hashes[i].execution_status;

            if (transaction.hash !== hashes[i].tx_hash) {
                console.error('Wrong transaction hash.');
                return;
            } else if (!type.verifySignature(transaction.body, transaction.signature, publicKeyOfTransaction)) {
                console.error('Wrong transaction signature.');
                return;
            }

            transactions.push(transaction);
        }

        return [data.block_info.block, wallet, transactions];
    }

}

CryptocurrencyService.prototype.getWallet = function(publicKey, callback) {
    var self = this;
    $.ajax({
        method: 'GET',
        url: this.baseUrl + '/wallets/info?pubkey=' + publicKey,
        success: function(response, textStatus, jqXHR) {
            callback.apply(this, self.validateWallet(publicKey, response));
        },
        error: function(jqXHR, textStatus, errorThrown) {
            console.error(textStatus);
        }
    });
};

CryptocurrencyService.prototype.addFunds = function(amount, publicKey, secretKey, callback) {
    var seed = Exonum.randomUint64();
    var data = {
        wallet: publicKey,
        amount: amount,
        seed: seed
    };

    this.submitTransaction(this.AddFundsTransactionParams, data, publicKey, secretKey, callback);
};

CryptocurrencyService.prototype.createWallet = function(login, password, callback) {
    var pair = Exonum.keyPair();
    var nonce = Exonum.randomNonce();
    var secretKeyEncrypted = Exonum.encryptDigest(pair.secretKey, nonce, password);
    var data = {
        login: login,
        pub_key: pair.publicKey,
        sec_key_enc: secretKeyEncrypted,
        nonce: nonce
    };
    
    this.submitTransaction(this.CreateWalletTransactionParams, data, pair.publicKey, pair.secretKey, callback);
};

CryptocurrencyService.prototype.transfer = function(amount, from, to, secretKey, callback) {
    var seed = Exonum.randomUint64();
    var data = {
        from: from,
        to: to,
        amount: amount,
        seed: seed
    };

    this.submitTransaction(this.TransferTransactionParams, data, from, secretKey, callback);
};

CryptocurrencyService.prototype.getBlocks = function(height, callback) {
    var suffix = '';
    if (!isNaN(height)) {
        suffix += '&from=' + height;
    }
    $.ajax({
        method: 'GET',
        url: this.baseUrl + '/blockchain/blocks?count=10' + suffix,
        success: callback,
        error: function(jqXHR, textStatus, errorThrown) {
            console.error(textStatus);
        }
    });
};

CryptocurrencyService.prototype.getBlock = function(height, callback) {
    var self = this;
    $.ajax({
        method: 'GET',
        url: this.baseUrl + '/blockchain/blocks/' + height,
        success: function(data, textStatus, jqXHR) {
            if (data && data.txs) {
                for (var i in data.txs) {
                    var type = self.getTransactionTypeParams(data.txs[i].message_id);
                    type.signature = data.txs[i].signature;
                    data.txs[i].hash = type.hash(data.txs[i].body);
                }
            }
            callback(data);
        },
        error: function(jqXHR, textStatus, errorThrown) {
            console.error(textStatus);
        }
    });
};

CryptocurrencyService.prototype.getTransaction = function(hash, callback) {
    $.ajax({
        method: 'GET',
        url: this.baseUrl + '/blockchain/transactions/' + hash,
        success: callback,
        error: function(jqXHR, textStatus, errorThrown) {
            console.error(textStatus);
        }
    });
};

CryptocurrencyService.prototype.login = function(login, password, callback, error) {
    $.ajax({
        method: 'GET',
        url: this.baseUrl + '/auth?login=' + login,
        success: function(data, textStatus, jqXHR) {
            var secretKey = Exonum.decryptDigest(data.sec_key_enc, data.nonce, password);
            if (secretKey !== false) {
                callback(data.pub_key, secretKey);
            } else {
                error();
            }
        },
        error: function(jqXHR, textStatus, errorThrown) {
            console.error(textStatus);
        }
    });
};
