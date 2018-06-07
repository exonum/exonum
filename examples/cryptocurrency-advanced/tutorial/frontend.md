# Cryptocurrency frontend tutorial

<!-- spell-checker:ignore uint -->

Cryptocurrency demo application is built on [Vue.js](https://vuejs.org)
framework together with [Bootstrap](https://getbootstrap.com/).

This tutorial covers the interaction of the client with Exonum blockchain:

- [Create a new user in the blockchain](#create-a-new-user)
- [Add funds to the user's balance](#add-funds)
- [Transfer funds between users](#transfer-funds)
- [Verify the proof of the user's existence](#verify-proof)

## Before start

There are two principal types of interaction between the client and
the blockchain.

*The first case* is when the client send some data to blockchain.
This data must be signed by the client's secret key before sending.
Such a signature can be verified by a blockchain before acceptance.
The data, together with its signature and service fields data, is called
**transaction**.

*The second case* is when the data is signed by the blockchain nodes
and the resulting signatures can be verified on the client.

## Create a new user

Generate a new key pair:

```javascript
const keyPair = Exonum.keyPair()
```

Define transaction:

```javascript
const TxCreateWallet = Exonum.newMessage({
  network_id: 0,
  protocol_version: 0,
  service_id: 128,
  message_id: 2,
  fields: [
    { name: 'pub_key', type: Exonum.PublicKey },
    { name: 'name', type: Exonum.String }
  ]
})
```

Prepare transaction data:

```javascript
const data = {
  pub_key: keyPair.publicKey,
  name: name
}
```

Sign data:

```javascript
const signature = TxCreateWallet.sign(keyPair.secretKey, data)
```

Submit transaction to blockchain:

```javascript
axios.post('/api/services/cryptocurrency/v1/wallets/transaction', {
  network_id: 0,
  protocol_version: 0,
  service_id: 128,
  message_id: 2,
  signature: signature,
  body: data
})
```

## Add funds

Define transaction:

```javascript
const TxIssue = Exonum.newMessage({
  network_id: 0,
  protocol_version: 0,
  service_id: 128,
  message_id: 1,
  fields: [
    { name: 'wallet', type: Exonum.PublicKey },
    { name: 'amount', type: Exonum.Uint64 },
    { name: 'seed', type: Exonum.Uint64 }
  ]
})
```

Generate random seed:

```javascript
const seed = Exonum.randomUint64()
```

Prepare transaction data:

```javascript
const data = {
  wallet: keyPair.publicKey,
  amount: amountToAdd.toString(),
  seed: seed
}
```

Sign data:

```javascript
const signature = TxIssue.sign(keyPair.secretKey, data)
```

Submit transaction to blockchain:

```javascript
axios.post('/api/services/cryptocurrency/v1/wallets/transaction', {
  network_id: 0,
  protocol_version: 0,
  service_id: 128,
  message_id: 1,
  signature: signature,
  body: data
})
```

## Transfer funds

Define transaction:

```javascript
const TxTransfer = Exonum.newMessage({
  network_id: 0,
  protocol_version: 0,
  service_id: 128,
  message_id: 0,
  fields: [
    { name: 'from', type: Exonum.PublicKey },
    { name: 'to', type: Exonum.PublicKey },
    { name: 'amount', type: Exonum.Uint64 },
    { name: 'seed', type: Exonum.Uint64 }
  ]
})
```

Generate random seed:

```javascript
const seed = Exonum.randomUint64()
```

Prepare transaction data:

```javascript
const data = {
  from: keyPair.publicKey,
  to: receiver,
  amount: amountToTransfer,
  seed: seed
}
```

Sign data:

```javascript
const signature = TxTransfer.sign(keyPair.secretKey, data)
```

Submit transaction to blockchain:

```javascript
axios.post(TX_URL, {
  network_id: 0,
  protocol_version: 0,
  service_id: 128,
  message_id: 0,
  signature: signature,
  body: data
})
```

## Verify proof

Here is how the proof of the user's existence looks:

```javascript
const data = {
  block_proof: {
    block: {...},
    precommits: [...]
  },
  wallet_proof: {
    to_table: {...},
    to_wallet: {...}
  },
  wallet_history: {
    proof: {...},
    transactions: [...]
  }
}
```

Verify the block and its precommits:

```javascript
if (Exonum.verifyBlock(data.block_proof, validators, 0)) {...}
```

`validators` is actual list of public keys of validators:

```javascript
const validators = [
  'a125c020442374014d3cd28f02674a8eb4114e7218c79a1158bdadb0c06b9919',
  ...
]
```

Extract the root hash of wallets tree from the Map proof of all tables (`data.wallet_proof.to_table`).

<!-- markdownlint-disable MD013 -->

```javascript
const tableProof = new Exonum.MapProof(data.wallet_proof.to_table, Exonum.Hash, Exonum.Hash)
```

Check either `state_hash` is equal to the `merkleRoot`.

```javascript
if (tableProof.merkleRoot !== data.block_proof.block.state_hash) {
  // throw error
}
```

Extract root hash of the wallets tree from the `tableProof`.

```javascript
const TableKey = Exonum.newType({
  fields: [
    { name: 'service_id', type: Exonum.Uint16 },
    { name: 'table_index', type: Exonum.Uint16 }
  ]
})

const tableKey = TableKey.hash({
  service_id: 0,
  table_index: 0
})

const walletsHash = tableProof.entries.get(tableKey)
```

Extract wallet from the tree of all wallets:

```javascript
const Wallet = Exonum.newType({
  fields: [
    { name: 'pub_key', type: Exonum.PublicKey },
    { name: 'name', type: Exonum.String },
    { name: 'balance', type: Exonum.Uint64 },
    { name: 'history_len', type: Exonum.Uint64 },
    { name: 'history_hash', type: Exonum.Hash }
  ]
})

const walletProof = new Exonum.MapProof(data.wallet_proof.to_wallet, Exonum.PublicKey, Wallet)
```

Compare `merkleRoot` with expected value.

```javascript
if (walletProof.merkleRoot !== walletsHash) {
  // throw error
}
```

Extract wallet data.

```javascript
const wallet = walletProof.entries.get(publicKey)
```

Extract transactions meta data from the Merkle tree.

```javascript
const transactionsMetaData = Exonum.merkleProof(
  wallet.history_hash,
  wallet.history_len,
  data.wallet_history.proof,
  [0, wallet.history_len],
  TransactionMetaData
)
```

Verify each transaction:

```javascript
for (let i = 0; i < data.wallet_history.transactions.length; i++) {
  let Transaction = getTransaction(data.wallet_history.transactions[i].message_id)
  const publicKeyOfTransaction = getPublicKeyOfTransaction(data.wallet_history.transactions[i].message_id, data.wallet_history.transactions[i].body)

  Transaction.signature = data.wallet_history.transactions[i].signature

  if (Transaction.hash(data.wallet_history.transactions[i].body) !== transactionsMetaData[i].tx_hash) {
    throw new Error('Invalid transaction hash has been found')
  }

  if (!Transaction.verifySignature(data.wallet_history.transactions[i].signature, publicKeyOfTransaction, data.wallet_history.transactions[i].body)) {
    throw new Error('Invalid transaction signature has been found')
  }
}
```

<!-- markdownlint-enable MD013 -->
`getTransaction` function returns element of `Exonum.newMessage` type.
