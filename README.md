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

1. Verify block

2. wallets table hash from merklePatricia tree

3. wallet data from wallets merklePatricia tree

4. hashes of all transactions from merkle tree

5. each hash is verified to make sure it match to transaction from array
