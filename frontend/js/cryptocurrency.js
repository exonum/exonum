/**
 * Business logic
 */
function Cryptocurrency(serviceId, validators) {

    var Transactions = [{
        size: 80,
        service_id: serviceId,
        message_id: 128,
        fields: {
            from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
            amount: {type: Exonum.Int64, size: 8, from: 64, to: 72},
            seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
        }
    }, {
        size: 48,
        service_id: serviceId,
        message_id: 129,
        fields: {
            wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            amount: {type: Exonum.Int64, size: 8, from: 32, to: 40},
            seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
        }
    }, {
        size: 40,
        service_id: serviceId,
        message_id: 130,
        fields: {
            pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
            name: {type: Exonum.String, size: 8, from: 32, to: 40}
        }
    }];
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

    function Transaction(messageId) {
        for (var i in Transactions) {
            if (!Transactions.hasOwnProperty(i)) {
                continue;
            } else if (Transactions[i].message_id === messageId) {
                return Exonum.newMessage(Transactions[i]); // add signature if defined
            }
        }

        console.error('Invalid message_id field');
        return;
    }

    function getPublicKeyOfTransactionOwner(transaction) {
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
            default:
                console.error('Invalid message_id field');
                return;
        }
    }

    function validateTransaction(transaction, hash) {
        var type = new Transaction(transaction.message_id);
        var publicKey = getPublicKeyOfTransactionOwner(transaction);

        type.signature = transaction.signature;

        if (type.hash(transaction.body) !== hash) {
            console.error('Wrong transaction hash.');
            return false;
        } else if (!type.verifySignature(transaction.body, transaction.signature, publicKey)) {
            console.error('Wrong transaction signature.');
            return false;
        }

        return true;
    }

    function getBlock(publicKey, data) {
        // validate block
        if (!Exonum.verifyBlock(data.block_info, validators)) {
            return undefined;
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
            service_id: serviceId,
            table_index: 0
        };
        var tableKey = TableKey.hash(tableKeyData);
        var walletsHash = Exonum.merklePatriciaProof(data.block_info.block.state_hash, data.wallet.mpt_proof, tableKey);
        if (walletsHash === null) {
            return undefined;
        }

        // find wallet in the tree of all wallets
        var wallet = Exonum.merklePatriciaProof(walletsHash, data.wallet.value, publicKey, Wallet);
        if (wallet === null) {
            return null;
        }

        // find hashes of all transactions
        var hashes = Exonum.merkleProof(wallet.history_hash, wallet.history_len, data.wallet_history.mt_proof, [0, wallet.history_len]);

        if (data.wallet_history.values.length !== hashes.length) {
            console.error('Number of transaction hashes is not equal to transactions number.');
            return undefined;
        }

        var transactions = [];
        for (var i in hashes) {
            if (!hashes.hasOwnProperty(i)) {
                continue;
            }

            if (!validateTransaction(data.wallet_history.values[i], hashes[i])) {
                return undefined;
            }

            var transaction = data.wallet_history.values[i];
            transaction.hash = hashes[i];

            transactions.push(transaction);
        }

        return {
            block: data.block_info.block,
            wallet: wallet,
            transactions: transactions
        };
    }
    
    function calculateHashesOfTransactions(transactions) {
        for (var i in transactions) {
            var type = new Transaction(transactions[i].message_id);

            type.signature = transactions[i].signature;
            transactions[i].hash = type.hash(transactions[i].body);
        }
    }

    function createWalletTransaction(name) {
        var pair = Exonum.keyPair();
        var data = {
            pub_key: pair.publicKey,
            name: name
        };
        var signature = Transaction(130).sign(data, pair.secretKey);
        var transaction = {
            service_id: serviceId,
            message_id: 130,
            body: data,
            signature: signature
        };
        return {
            pair: pair,
            transaction: transaction
        }
    }

    function addFundsTransaction(amount, wallet, secretKey) {
        var seed = Exonum.randomUint64();
        var data = {
            wallet: wallet,
            amount: amount,
            seed: seed
        };
        var signature = Transaction(129).sign(data, secretKey);

        return {
            service_id: serviceId,
            message_id: 129,
            body: data,
            signature: signature
        };
    }

    function transferTransaction(amount, from, to, secretKey) {
        var seed = Exonum.randomUint64();
        var data = {
            from: from,
            to: to,
            amount: amount,
            seed: seed
        };
        var signature = Transaction(128).sign(data, secretKey);

        return {
            service_id: serviceId,
            message_id: 128,
            body: data,
            signature: signature
        };
    }

    return {
        Transaction: Transaction,
        getBlock: getBlock,
        calculateHashesOfTransactions: calculateHashesOfTransactions,
        createWalletTransaction: createWalletTransaction,
        addFundsTransaction: addFundsTransaction,
        transferTransaction: transferTransaction
    };

}