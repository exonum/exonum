import * as Exonum from 'exonum-client'
import * as Protobuf from 'protobufjs/light'
import axios from 'axios'

const TRANSACTION_URL = '/api/explorer/v1/transactions'
const PER_PAGE = 10
const SERVICE_ID = 128
const TX_TRANSFER_ID = 0
const TX_ISSUE_ID = 1
const TX_WALLET_ID = 2
const TABLE_INDEX = 0
const Wallet = Exonum.newType({
  fields: [
    { name: 'pub_key', type: Exonum.PublicKey },
    { name: 'name', type: Exonum.String },
    { name: 'balance', type: Exonum.Uint64 },
    { name: 'history_len', type: Exonum.Uint64 },
    { name: 'history_hash', type: Exonum.Hash }
  ]
})
const TransactionMetaData = Exonum.newType({
  fields: [
    { name: 'tx_hash', type: Exonum.Hash },
    { name: 'execution_status', type: Exonum.Bool }
  ]
})
const Type = Protobuf.Type;
const Field = Protobuf.Field;

function TransferTransaction(publicKey) {
  const Transfer = new Type('Transfer');
  Transfer.add(new Field('to', 1, 'bytes'));
  Transfer.add(new Field('amount', 2, 'uint64'));
  Transfer.add(new Field('seed', 3, 'uint64'));
  return Exonum.newTransaction({
    author: publicKey,
    service_id: SERVICE_ID,
    message_id: TX_TRANSFER_ID,
    schema: Transfer
  })
}

function IssueTransaction(publicKey) {
  const Issue = new Type('Transfer');
  Issue.add(new Field('amount', 1, 'uint64'));
  Issue.add(new Field('seed', 2, 'uint64'));
  return Exonum.newTransaction({
    author: publicKey,
    service_id: SERVICE_ID,
    message_id: TX_ISSUE_ID,
    schema: Issue
  })
}

function CreateTransaction(publicKey) {
  const Create = new Type('Transfer');
  Create.add(new Field('name', 1, 'string'));
  return Exonum.newTransaction({
    author: publicKey,
    service_id: SERVICE_ID,
    message_id: TX_WALLET_ID,
    schema: Create
  })
}

function getTransaction(transaction, publicKey) {
  if (transaction.name) {
    return new CreateTransaction(publicKey)
  }

  if (transaction.to) {
    return new TransferTransaction(publicKey)
  }

  return new IssueTransaction(publicKey)
}

module.exports = {
  install(Vue) {
    Vue.prototype.$blockchain = {
      generateKeyPair() {
        return Exonum.keyPair()
      },

      generateSeed() {
        return Exonum.randomUint64()
      },

      createWallet(keyPair, name) {
        // Describe transaction
        const transaction = new CreateTransaction(keyPair.publicKey)

        // Transaction data
        const data = {
          name: name
        }

        // Send transaction into blockchain
        return transaction.send(TRANSACTION_URL, data, keyPair.secretKey)
      },

      addFunds(keyPair, amountToAdd, seed) {
        // Describe transaction
        const transaction = new IssueTransaction(keyPair.publicKey)

        // Transaction data
        const data = {
          amount: amountToAdd.toString(),
          seed: seed
        }

        // Send transaction into blockchain
        return transaction.send(TRANSACTION_URL, data, keyPair.secretKey)
      },

      transfer(keyPair, receiver, amountToTransfer, seed) {
        // Describe transaction
        const transaction = new TransferTransaction(keyPair.publicKey)

        // Transaction data
        const data = {
          to: Uint8Array.from(Exonum.hexadecimalToUint8Array(receiver)),
          amount: amountToTransfer,
          seed: seed
        }

        // Send transaction into blockchain
        return transaction.send(TRANSACTION_URL, data, keyPair.secretKey)
      },

      getWallet(publicKey) {
        return axios.get('/api/services/configuration/v1/configs/actual').then(response => {
          // actual list of public keys of validators
          const validators = response.data.config.validator_keys.map(validator => {
            return validator.consensus_key
          })

          return axios.get(`/api/services/cryptocurrency/v1/wallets/info?pub_key=${publicKey}`)
            .then(response => response.data)
            .then(data => {
              if (!Exonum.verifyBlock(data.block_proof, validators)) {
                throw new Error('Block can not be verified')
              }

              // verify table timestamps in the root tree
              const tableRootHash = Exonum.verifyTable(data.wallet_proof.to_table, data.block_proof.block.state_hash, SERVICE_ID, TABLE_INDEX)

              // find wallet in the tree of all wallets
              const walletProof = new Exonum.MapProof(data.wallet_proof.to_wallet, Exonum.PublicKey, Wallet)
              if (walletProof.merkleRoot !== tableRootHash) {
                throw new Error('Wallet proof is corrupted')
              }
              const wallet = walletProof.entries.get(publicKey)
              if (typeof wallet === 'undefined') {
                throw new Error('Wallet not found')
              }

              // get transactions
              const transactionsMetaData = Exonum.merkleProof(
                wallet.history_hash,
                wallet.history_len,
                data.wallet_history.proof,
                [0, wallet.history_len],
                TransactionMetaData
              )

              if (data.wallet_history.transactions.length !== transactionsMetaData.length) {
                // number of transactions in wallet history is not equal
                // to number of transactions in array with transactions meta data
                throw new Error('Transactions can not be verified')
              }

              // validate each transaction
              const transactions = []
              let index = 0
              for (let transaction of data.wallet_history.transactions) {
                const hash = transactionsMetaData[index++]
                const buffer = Exonum.hexadecimalToUint8Array(transaction.message)
                const bufferWithoutSignature = buffer.subarray(0, buffer.length - 64)
                const author = Exonum.uint8ArrayToHexadecimal(buffer.subarray(0, 32))
                const signature = Exonum.uint8ArrayToHexadecimal(buffer.subarray(buffer.length - 64, buffer.length));

                const Transaction = getTransaction(transaction.debug, author)

                if (Exonum.hash(buffer) !== hash) {
                  throw new Error('Invalid transaction hash')
                }

                // serialize transaction and compare with message
                if (!Transaction.serialize(transaction.debug).every(function (el, i) {
                  return el === bufferWithoutSignature[i]
                })) {
                  throw new Error('Invalid transaction message')
                }

                if (!Transaction.verifySignature(signature, author, transaction.debug)) {
                  throw new Error('Invalid transaction signature')
                }

                transactions.push(Object.assign({ hash: hash }, transaction.debug))
              }

              return {
                block: data.block_proof.block,
                wallet: wallet,
                transactions: transactions
              }
            })
        })
      },

      getBlocks(latest) {
        const suffix = !isNaN(latest) ? '&latest=' + latest : ''
        return axios.get(`/api/explorer/v1/blocks?count=${PER_PAGE}${suffix}`).then(response => response.data)
      },

      getBlock(height) {
        return axios.get(`/api/explorer/v1/block?height=${height}`).then(response => response.data)
      },

      getTransaction(hash) {
        return axios.get(`/api/explorer/v1/transactions?hash=${hash}`).then(response => response.data)
      }
    }
  }
}
