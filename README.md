# Crypto currency demo

This is demo of crypto currency implemented on Exonum blockchain.

It demonstrates the very basic operations:

- create a new wallet

- add funds into a wallet

- transfer funds from the one wallet to another

- monitor blocks status

### Backend

TODO

### Frontend

Frontend is a lightweight single page application implemented on [riotjs](https://github.com/riot/riot).

Application is served by Node.js and communicates directly with backends REST api and uses Exonum client to convert data into appropriate format and parse it into JSON.

#### Submit transaction

To create transaction the new entity of `newMessage` type should be declared.

Here is example of how transfer transaction is declared:

```
var TransferTransaction = {
    size: 80,
    service_id: 128,
    message_id: 128,
    fields: {
        from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
        to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
        amount: {type: Exonum.Int64, size: 8, from: 64, to: 72},
        seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
    }
};
```

Then transaction data is signed:

```
var seed = Exonum.randomUint64();

var data = {
    from: ...,
    to: ...,
    amount: ...,
    seed: seed
};

var signature = TransferTransaction.sign(data, secretKey);
```

Finally, signed data and signature are submitted to server:

```
{
    service_id: 128,
    message_id: 128,
    body: data,
    signature: signature
}
```

#### Get wallet

Backend returns wallet info in block with precommits.

Here the list of necessary steps:

1) Block can be verified with Exonum client:

```javascript
Exonum.verifyBlock(data.block_info, validators);
```

`validators` is the array of validators.

2) Wallets table hash can be found at `wallet.mpt_proof` Merkle Patricia tree. Key for value is generated using `service_id` and `table_index`:

```javascript
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
```

3) Wallet's data can be found at `wallet.value` Merkle Patricia tree. Wallets table hash from previous step is used as key.

```
var wallet = Exonum.merklePatriciaProof(walletsHash, data.wallet.value, publicKey, Wallet);
```

`publicKey` is the public key of wallet;

`Wallet` is the custom type:

```javascript
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
```

4) Hashes of all transactions can be found at `wallet_history.mt_proof` Merkle tree.

5) List of all transactions can be found at  `wallet.values`. Each transaction can be compared with hash from previous step.
