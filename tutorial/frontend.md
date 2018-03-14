# Cryptocurrency frontend tutorial

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
  size: 40,
  network_id: 0,
  protocol_version: 0,
  service_id: 128,
  message_id: 130,
  fields: {
    pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
    name: {type: Exonum.String, size: 8, from: 32, to: 40}
  }
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
  message_id: 130,
  signature: signature,
  body: data
})
```

## Add funds

Define transaction:

```javascript
const TxIssue = Exonum.newMessage({
  size: 48,
  network_id: 0,
  protocol_version: 0,
  service_id: 128,
  message_id: 129,
  fields: {
    wallet: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
    amount: {type: Exonum.Uint64, size: 8, from: 32, to: 40},
    seed: {type: Exonum.Uint64, size: 8, from: 40, to: 48}
  }
})
```

Prepare transaction data:

```javascript
const data = {
  wallet: keyPair.publicKey,
  amount: amountToAdd.toString(),
  seed: Exonum.randomUint64()
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
  message_id: 129,
  signature: signature,
  body: data
})
```

## Transfer funds

Define transaction:

```javascript
const TxTransfer = Exonum.newMessage({
  size: 80,
  network_id: 0,
  protocol_version: 0,
  service_id: 128,
  message_id: 128,
  fields: {
    from: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
    to: {type: Exonum.PublicKey, size: 32, from: 32, to: 64},
    amount: {type: Exonum.Uint64, size: 8, from: 64, to: 72},
    seed: {type: Exonum.Uint64, size: 8, from: 72, to: 80}
  }
})
```

Prepare transaction data:

```javascript
const data = {
  from: keyPair.publicKey,
  to: receiver,
  amount: amountToTransfer,
  seed: Exonum.randomUint64()
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
  message_id: 128,
  signature: signature,
  body: data
})
```

## Verify proof

Here is how the proof of the user's existence looks:

```javascript
const data = {
  block_info: {
    block: {...},
    precommits: [...]
  },
  wallet: {
    mpt_proof: {...},
    value: {...}
  },
  wallet_history: {
    mt_proof: {...},
    values: [...]
  }
}
```

Verify the block and its precommits:

```javascript
if (Exonum.verifyBlock(data.block_info, validators, 0)) {...}
```

`validators` is actual list of public keys of validators:

```javascript
const validators = [
  'a125c020442374014d3cd28f02674a8eb4114e7218c79a1158bdadb0c06b9919',
  ...
]
```

Extract the value from the Merkle Patricia tree of all tables (`data.wallet.mpt_proof`).
Use `state_hash` as a root hash of the tree.

```javascript
const TableKey = Exonum.newType({
  size: 4,
  fields: {
    service_id: {type: Exonum.Uint16, size: 2, from: 0, to: 2},
    table_index: {type: Exonum.Uint16, size: 2, from: 2, to: 4}
  }
})

const tableKey = TableKey.hash({
  service_id: 0,
  table_index: 0
})

const walletsHash = Exonum.merklePatriciaProof(data.block_info.block.state_hash, data.wallet.mpt_proof, tableKey)
```

Extracted value is a root hash of the wallets Merkle Patricia tree (`data.wallet.value`).
Extract wallet from the tree:

```javascript
const Wallet = Exonum.newType({
  size: 88,
  fields: {
    pub_key: {type: Exonum.PublicKey, size: 32, from: 0, to: 32},
    name: {type: Exonum.String, size: 8, from: 32, to: 40},
    balance: {type: Exonum.Uint64, size: 8, from: 40, to: 48},
    history_len: {type: Exonum.Uint64, size: 8, from: 48, to: 56},
    history_hash: {type: Exonum.Hash, size: 32, from: 56, to: 88}
  }
})

const wallet = Exonum.merklePatriciaProof(walletsHash, data.wallet.value, publicKey, Wallet)
```

Extract transactions meta data from the Merkle tree:

```javascript
const transactionsMetaData = Exonum.merkleProof(
  wallet.history_hash,
  wallet.history_len,
  data.wallet_history.mt_proof,
  [0, wallet.history_len],
  TransactionMetaData
)
```

Verify each transaction:

```javascript
for (let i = 0; i < data.wallet_history.values.length; i++) {
  let Transaction = getTransaction(data.wallet_history.values[i].message_id)
  const publicKeyOfTransaction = getPublicKeyOfTransaction(data.wallet_history.values[i].message_id, data.wallet_history.values[i].body)

  Transaction.signature = data.wallet_history.values[i].signature

  if (Transaction.hash(data.wallet_history.values[i].body) !== transactionsMetaData[i].tx_hash) {
    throw new Error('Invalid transaction hash has been found')
  }

  if (!Transaction.verifySignature(data.wallet_history.values[i].signature, publicKeyOfTransaction, data.wallet_history.values[i].body)) {
    throw new Error('Invalid transaction signature has been found')
  }
}
```

`getTransaction` function returns element of `Exonum.newMessage` type.
