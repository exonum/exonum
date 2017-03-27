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

All business logic is can be found in the file `cryptocurrency.js`.

#### Submit transaction

To create transaction of each type you need to declare the new entity of `newMessage` type.

##### Create a new wallet transaction

Here is an example of how `create a new wallet` transaction is declared:

```
var CreateWalletTransaction = {
    size: 40,
    service_id: 128,
    message_id: 130,
    fields: {
        pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
        name: {type: Exonum.String, size: 8, from: 32, to: 40}
    }
};
```

Then new random pair of publicKey and secretKey is generated:

```javascript
var pair = Exonum.keyPair(); 
```

Then transaction data can be signed:

```
var data = {
    pub_key: pair.publicKey,
    name: ...
};

var signature = CreateWalletTransaction.sign(data, pair.secretKey);
```

Finally, signed data and signature can be submitted to server:

```
{
    service_id: 128,
    message_id: 130,
    body: data,
    signature: signature
}
```

##### Add funds transaction

Here is an example of how `add funds` transaction is declared:

```
var AddFundsTransaction = {
    size: 48,
    service_id: 128,
    message_id: 129,
    fields: {
        wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
        amount: {type: Exonum.Int64, size: 8, from: 32, to: 40},
        seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
    }
};
```

Then transaction data can be signed:

```
var seed = Exonum.randomUint64();

var data = {
    wallet: ...,
    amount: ...,
    seed: seed
};

var signature = TransferTransaction.sign(data, secretKey);
```

Finally, signed data and signature can be submitted to server:

```
{
    service_id: 128,
    message_id: 129,
    body: data,
    signature: signature
}
```

##### Transfer transaction

Here is an example of how `transfer` transaction is declared:

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

Then transaction data can be signed:

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

Finally, signed data and signature can be submitted to server:

```
{
    service_id: 128,
    message_id: 128,
    body: data,
    signature: signature
}
```

#### Get wallet

Backend returns wallet info as a block with precommits.

Here the list of a necessary steps:

1) Verify block:

```javascript
Exonum.verifyBlock(data.block_info, validators);
```

`validators` is the array of validators.

2) Find wallets table hash at Merkle Patricia tree stored in `wallet.mpt_proof`. Key of this value is generated using `service_id` and `table_index`:

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

3) Find wallet's data at Merkle Patricia tree stored in `wallet.value`. Wallets table hash from previous step is used as key.

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

4) Find hashes of all transactions at Merkle tree in `wallet_history.mt_proof`.

5) Find list of all transactions at array stored in `wallet.values`. Each transaction comparing with hash from previous step.

The steps from above guarantees all wallet info reliability and consistency.
