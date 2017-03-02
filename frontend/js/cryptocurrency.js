function Cryptocurrency() {
    var transactions = {
        128: Exonum.newMessage({
            size: 80,
            service_id: 128,
            message_id: 128,
            fields: {
                from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
                amount: {type: Exonum.Int64, size: 8, from: 64, to: 72},
                seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
            }
        }),
        129: Exonum.newMessage({
            size: 48,
            service_id: 128,
            message_id: 129,
            fields: {
                wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                amount: {type: Exonum.Int64, size: 8, from: 32, to: 40},
                seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
            }
        }),
        130: Exonum.newMessage({
            size: 40,
            service_id: 128,
            message_id: 130,
            fields: {
                pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
                name: {type: Exonum.String, size: 8, from: 32, to: 40}
            }
        })
    };
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

    function verifyTransaction(transaction, hash) {
        var Type = transactions[transaction.message_id];
        var publicKey;

        switch (transaction.message_id) {
            case 128:
                publicKey = transaction.body.from;
                break;
            case 129:
                publicKey = transaction.body.wallet;
                break;
            case 130:
                publicKey = transaction.body.pub_key;
                break;
            default:
                console.error('Invalid message_id field');
                return false;
        }

        if (Exonum.hash(transaction.body, Type) !== hash) {
            console.error('Wrong transaction hash.');
            return false;
        } else if (!Exonum.verifySignature(transaction.body, Type, transaction.signature, publicKey)) {
            console.error('Wrong transaction signature.');
            return false;
        }

        return true;
    }

    function parseBlock(publicKey, validators, response) {
        if (!Exonum.verifyBlock(response.block_info, validators)) {
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
        var walletsTableRootHash = Exonum.merklePatriciaProof(response.block_info.block.state_hash, response.wallet.mpt_proof, walletsTableKey);
        if (walletsTableRootHash === null) {
            console.error('Wallets can not exist.');
            return undefined;
        }

        var wallet = Exonum.merklePatriciaProof(walletsTableRootHash, response.wallet.value, publicKey, Wallet);
        if (wallet === null) {
            return null;
        }

        var HashesOftransactions = Exonum.merkleProof(wallet.history_hash, wallet.history_len, response.wallet_history.mt_proof, [0, wallet.history_len]);
        var transactions = response.wallet_history.values;

        if (transactions.length !== HashesOftransactions.length) {
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

            transactions[i].hash = HashesOftransactions[i]; // TODO do a separate API method; it is also required in other services
        }

        return {
            wallet: wallet,
            transactions: transactions
        };
    }

    return {
        transactions: transactions,
        Wallet: Wallet,
        parseBlock: parseBlock
    };
}