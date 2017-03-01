<wallet>
    <div class="text-center">
        <h2 if={ balance }>${ balance }</h2>
        <h6 if={ blockHeight }>Block #<a href="/#blockchain/block/{ blockHeight }">{ blockHeight }</a></h6>
        <h6 if={ blockTime }>{ blockTime }</h6>
    </div>

    <div class="text-center">
        <div if={ balance === 0 } class="alert alert-warning">
            <i class="glyphicon glyphicon-alert"></i> You haven't any money yet. Add some funds.
        </div>
        <div class="form-group">
            <button class="btn btn-lg btn-primary" disabled={ balance == 0 } onclick={ transfer }>Transfer</button>
            <a href="/#user/{ opts.publicKey }/add-funds" class="btn btn-lg btn-success">Add Funds</a>
        </div>
    </div>

    <legend class="text-center">Transactions history</legend>

    <table class="table table-striped">
        <thead>
        <tr>
            <th>Hash</th>
            <th>Description</th>
        </tr>
        </thead>
        <tbody>

        <tr each={ transactions }>
            <td>
                <a href="/#blockchain/transaction/{ hash }">{ hash }</a>
            </td>
            <td if={message_id === 130}>
                create wallet
            </td>
            <td if={message_id === 129}>
                add <strong>${ body.amount }</strong> to your wallet
            </td>
            <td if={message_id === 128 && body.from === parent.opts.publicKey}>
                send <strong>${ body.amount }</strong> to <a href="/#user/{ body.to }">{ body.to }</a>
            </td>
            <td if={message_id === 128 && body.to === parent.opts.publicKey}>
                receive <strong>${ body.amount }</strong> from <a href="/#user/{ body.from }">{ body.from }</a>
            </td>
        </tr>

        </tbody>
    </table>

    <a class="btn btn-lg btn-block btn-default" href="/#">Log out</a>

    <script>
        var self = this;

        // TODO refactor
        var validators = [
            '7e2b6889b2e8b60e0e8d71be55b9cbf6aaa9bf397ef7b1d6b8564d862b120bea',
            '2f1e58c0752503e3b66a5f68d97ab44cac196c75608b53682c3da1f824f9391f',
            '8ce8ba0974e10d45d89b48a409015ebfe15a4aa9f9410951b266764b91c9d535',
            '11110c9c4b06d7cc0df9311aae089771b04b696a8eaa105ba39a186bcceed0c2'
        ];

        transfer(e) {
            route('/user/' + self.opts.publicKey + '/transfer');
        }

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
                }

                if (!verifyTransaction(transactions[i], HashesOftransactions[i])) {
                    return undefined;
                }

                transactions[i].hash = HashesOftransactions[i];
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

                self.opts.titleObservable.trigger('change', walletData.wallet.name);

                self.balance = walletData.wallet.balance;
                self.blockHeight = data.block_info.block.height;
                self.blockTime = data.block_info.block.time;
                self.transactions = walletData.transactions;
                self.update();

//                for (var i = 0; i < .length; i++) {
//                    switch(walletData.transactions[i].message_id) {
//                        case 128:
//                            if (walletData.transactions[i].body.from === self.opts.publicKey) {
//                                riot.mount('#wallet-transactions', 'wallet-transaction-send-transfer', {
//                                    hash: Exonum.hash(walletData.transactions[i].body, getTransactionType(walletData.transactions[i])),
//                                    amount: walletData.transactions[i].body.amount,
//                                    to: walletData.transactions[i].body.to
//                                });
//                            } else {
//                                riot.mount('#wallet-transactions', 'wallet-transaction-receive-transfer', {
//                                    hash: Exonum.hash(walletData.transactions[i].body, getTransactionType(walletData.transactions[i])),
//                                    amount: walletData.transactions[i].body.amount,
//                                    from: walletData.transactions[i].body.from
//                                });
//                            }
//                            break;
//                        case 129:
//                            riot.mount('#wallet-transactions', 'wallet-transaction-add-funds', {
//                                hash: Exonum.hash(walletData.transactions[i].body, getTransactionType(walletData.transactions[i])),
//                                amount: walletData.transactions[i].body.amount
//                            });
//                            break;
//                        case 130:riot.mount('#wallet-transactions', 'wallet-transaction-create-wallet', {
//                                hash: Exonum.hash(walletData.transactions[i].body, getTransactionType(walletData.transactions[i]))
//                            });
//                            break;
//                    }
//
//                }
            },
            error: function(jqXHR, textStatus, errorThrown) {
                console.error(textStatus);
            }
        });
    </script>
</wallet>