import * as Exonum from 'exonum-client'
import axios from 'axios'
import * as proto from '../../proto/stubs.js'

const TRANSACTION_URL = '/api/explorer/v1/transactions'
const PER_PAGE = 10
const SERVICE_ID = 128
const TX_TRANSFER_ID = 0
const TX_ISSUE_ID = 1
const TX_WALLET_ID = 2
const TABLE_INDEX = 0
const Wallet = Exonum.newType(proto.exonum.examples.cryptocurrency_advanced.Wallet)

function TransferTransaction(publicKey) {
  return Exonum.newTransaction({
    author: publicKey,
    service_id: SERVICE_ID,
    message_id: TX_TRANSFER_ID,
    schema: proto.exonum.examples.cryptocurrency_advanced.Transfer
  })
}

function IssueTransaction(publicKey) {
  return Exonum.newTransaction({
    author: publicKey,
    service_id: SERVICE_ID,
    message_id: TX_ISSUE_ID,
    schema: proto.exonum.examples.cryptocurrency_advanced.Issue
  })
}

function CreateTransaction(publicKey) {
  return Exonum.newTransaction({
    author: publicKey,
    service_id: SERVICE_ID,
    message_id: TX_WALLET_ID,
    schema: proto.exonum.examples.cryptocurrency_advanced.CreateWallet
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
          to: { data: Exonum.hexadecimalToUint8Array(receiver) },
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
              return Exonum.verifyBlock(data.block_proof, validators).then(() => {
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
                  Exonum.uint8ArrayToHexadecimal(new Uint8Array(wallet.history_hash.data)),
                  wallet.history_len,
                  data.wallet_history.proof,
                  [0, wallet.history_len],
                  Exonum.Hash
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

                  const transactionData = Object.assign({ hash: hash }, transaction.debug)
                  if (transactionData.to) {
                    transactionData.to = Exonum.uint8ArrayToHexadecimal(new Uint8Array(transactionData.to.data))
                  }
                  transactions.push(transactionData)
                }

                return {
                  block: data.block_proof.block,
                  wallet: wallet,
                  transactions: transactions
                }
              })
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
