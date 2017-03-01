<add-funds>
    <virtual if={ !succeed }>
        <div class="text-center">
            <h2 if={ balance }>${ balance }</h2>
            <h6 if={ blockHeight }>Block #<a href="/#blockchain/block/{ blockHeight }">{ blockHeight }</a></h6>
        </div>

        <legend class="text-center">Add funds</legend>
        <button type="submit" class="btn btn-lg btn-block btn-success" onclick={ addFunds } data-amount="10">Add $10.00</button>
        <button type="submit" class="btn btn-lg btn-block btn-success" onclick={ addFunds } data-amount="50">Add $50.00</button>
        <button type="submit" class="btn btn-lg btn-block btn-success" onclick={ addFunds } data-amount="100">Add $100.00</button>
    </virtual>

    <p if={ succeed } class="lead text-center">Excellent! Funds will be transfered in a seconds.</p>

    <a class="btn btn-lg btn-block btn-default" href="/#user/{ opts.publicKey }">Back</a>

    <script>
        var self = this;

        this.title = 'Add Funds';

        addFunds(e) {
            e.preventDefault();
            var amount = $(e.target).data('amount').toString();
            var secretKey;
            var users = JSON.parse(window.localStorage.getItem('users'));
            for (var i = 0; i < users.length; i++) {
                if (users[i].publicKey === self.opts.publicKey) {
                    secretKey = users[i].secretKey
                }
            }
            // TODO move outside
            var TxAddFunds = Exonum.newMessage({
                size: 48,
                service_id: 128,
                message_id: 129,
                fields: {
                    wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                    amount: {type: Exonum.Uint64, size: 8, from: 32, to: 40},
                    seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
                }
            });
            var seed = Exonum.randomUint64();
            var data = {
                wallet: self.opts.publicKey,
                amount: amount,
                seed: seed
            };
            var signature = Exonum.sign(data, TxAddFunds, secretKey);

            $.ajax({
                method: 'POST',
                url: self.api.baseUrl + '/wallets/transaction',
                contentType: 'application/json',
                data: JSON.stringify({
                    service_id: 128,
                    message_id: 129,
                    body: data,
                    signature: signature
                }),
                success: function(data, textStatus, jqXHR) {
                    self.succeed = true;

                    $.ajax({
                        method: 'GET',
                        url: self.api.baseUrl + '/wallets/info?pubkey=' + self.opts.publicKey,
                        success: function(data, textStatus, jqXHR) {
                            var walletData = getWallet(data, self.opts.publicKey);

                            self.balance = walletData.wallet.balance;
                            self.update();
                        },
                        error: function(jqXHR, textStatus, errorThrown) {
                            console.error(textStatus);
                        }
                    });
                },
                error: function(jqXHR, textStatus, errorThrown) {
                    console.error(textStatus);
                }
            });
        }

        var validators = [
            '7e2b6889b2e8b60e0e8d71be55b9cbf6aaa9bf397ef7b1d6b8564d862b120bea',
            '2f1e58c0752503e3b66a5f68d97ab44cac196c75608b53682c3da1f824f9391f',
            '8ce8ba0974e10d45d89b48a409015ebfe15a4aa9f9410951b266764b91c9d535',
            '11110c9c4b06d7cc0df9311aae089771b04b696a8eaa105ba39a186bcceed0c2'
        ];

        function getTransactionType(transaction) {
            switch (transaction.message_id) {
                case 128:
                    return Exonum.newMessage({
                        size: 80,
                        service_id: 128,
                        message_id: 128,
                        signature: transaction.signature,
                        fields: {
                            from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
                            amount: {type: Exonum.Int64, size: 8, from: 64, to: 72},
                            seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
                        }
                    });
                    break;
                case 129:
                    return Exonum.newMessage({
                        size: 48,
                        service_id: 128,
                        message_id: 129,
                        signature: transaction.signature,
                        fields: {
                            wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            amount: {type: Exonum.Int64, size: 8, from: 32, to: 40},
                            seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
                        }
                    });
                case 130:
                    return Exonum.newMessage({
                        size: 40,
                        service_id: 128,
                        message_id: 130,
                        signature: transaction.signature,
                        fields: {
                            pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                            name: {type: Exonum.String, size: 8, from: 32, to: 40}
                        }
                    });
                    break;
            }
        }

        function getTransationPublicKey(transaction) {
            switch (transaction.message_id) {
                case 128:
                    return transaction.body.from;
                    break;
                case 129:
                    return transaction.body.wallet;
                    break;
                case 130:
                    return transaction.body.pub_key;
                    break;
            }
        }

        function verifyTransaction(transaction, hash) {
            var Type = getTransactionType(transaction);
            var publicKey = getTransationPublicKey(transaction);

            if (Exonum.hash(transaction.body, Type) !== hash) {
                console.error('Wrong transaction hash.');
                return false;
            } else if (!Exonum.verifySignature(transaction.body, Type, transaction.signature, publicKey)) {
                console.error('Wrong transaction signature.');
                return false;
            }

            return true;
        }

        function getObjectLength(obj) {
            var l = 0;
            for (var prop in obj) {
                if (obj.hasOwnProperty(prop)) {
                    l++;
                }
            }
            return l;
        }

        function getWallet(query, publicKey) {
            if (!Exonum.verifyBlock(query.block_info, validators)) {
                return undefined;
            }

            var TableKey = Exonum.newType({
                size: 4,
                fields: {
                    service_id: {type: Exonum.Uint16, size: 2, from: 0, to: 2},
                    table_index: {type: Exonum.Uint16, size: 2, from: 2, to: 4}
                }
            });
            var walletsTableData = {
                service_id: 128,
                table_index: 0
            };
            var walletsTableKey = Exonum.hash(walletsTableData, TableKey);
            var walletsTableRootHash = Exonum.merklePatriciaProof(query.block_info.block.state_hash, query.wallet.mpt_proof, walletsTableKey);

            if (walletsTableRootHash === null) {
                console.error('Wallets can not exist.');
                return undefined;
            }

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

            var wallet = Exonum.merklePatriciaProof(walletsTableRootHash, query.wallet.value, publicKey, Wallet);

            if (wallet === null) {
                // wallet not found
                return null;
            }

            var HashesOftransactions = Exonum.merkleProof(wallet.history_hash, wallet.history_len, query.wallet_history.mt_proof, [0, wallet.history_len]);
            var transactions = query.wallet_history.values;

            if (getObjectLength(transactions) !== getObjectLength(HashesOftransactions)) {
                console.error('Number of transaction hashes is not equal to transactions number.');
                return undefined;
            }

            for (var i in HashesOftransactions) {
                if (!HashesOftransactions.hasOwnProperty(i)) {
                    continue;
                } if (!verifyTransaction(transactions[i], HashesOftransactions[i])) {
                    return undefined;
                }
            }

            return {
                wallet: wallet,
                transactions: transactions
            };
        }

        $.ajax({
            method: 'GET',
            url: this.api.baseUrl + '/wallets/info?pubkey=' + self.opts.publicKey,
            success: function(data, textStatus, jqXHR) {
                var walletData = getWallet(data, self.opts.publicKey);

                self.balance = walletData.wallet.balance;
                self.blockHeight = data.block_info.block.height;
                self.update();
            },
            error: function(jqXHR, textStatus, errorThrown) {
                console.error(textStatus);
            }
        });
    </script>
</add-funds>