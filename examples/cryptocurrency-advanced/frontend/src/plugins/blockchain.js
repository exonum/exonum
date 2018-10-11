import * as Exonum from 'exonum-client'
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

function TransferTransaction (publicKey) {
  return Exonum.newTransaction({
    author: publicKey,
    service_id: SERVICE_ID,
    message_id: TX_TRANSFER_ID,
    fields: [
      { name: 'to', type: Exonum.PublicKey },
      { name: 'amount', type: Exonum.Uint64 },
      { name: 'seed', type: Exonum.Uint64 }
    ]
  })
}

function IssueTransaction (publicKey) {
  return Exonum.newTransaction({
    author: publicKey,
    service_id: SERVICE_ID,
    message_id: TX_ISSUE_ID,
    fields: [
      { name: 'amount', type: Exonum.Uint64 },
      { name: 'seed', type: Exonum.Uint64 }
    ]
  })
}

function CreateTransaction (publicKey) {
  return Exonum.newTransaction({
    author: publicKey,
    service_id: SERVICE_ID,
    message_id: TX_WALLET_ID,
    fields: [
      { name: 'name', type: Exonum.String }
    ]
  })
}

function getTransaction(key, publicKey) {
  switch (key) {
    case 'Transfer':
      return new TransferTransaction(publicKey)
    case 'Issue':
      return new IssueTransaction(publicKey)
    case 'CreateWallet':
      return new CreateTransaction(publicKey)
    default:
      throw new Error('Unknown transaction name has been passed')
  }
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
          to: receiver,
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
              let transactions = []
              let index = 0
              for (let transaction of data.wallet_history.transactions) {
                // get transaction definition
                let Transaction = getTransaction(name, publicKey)

                // add a signature to the transaction definition
                Transaction.signature = transaction.signature

                // validate transaction hash
                if (Transaction.hash(transaction.body) !== transactionsMetaData[index]) {
                  throw new Error('Invalid transaction hash has been found')
                }

                // validate transaction signature
                if (!Transaction.verifySignature(transaction.signature, transaction.author, transaction.body)) {
                  throw new Error('Invalid transaction signature has been found')
                }

                // add transaction to the resulting array
                transactions.push(Object.assign({
                  hash: transactionsMetaData[index]
                }, transaction))

                index++
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
