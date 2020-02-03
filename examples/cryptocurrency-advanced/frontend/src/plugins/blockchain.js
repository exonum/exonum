import * as Exonum from 'exonum-client'
import axios from 'axios'
import * as proto from '../../proto/stubs.js'

const TRANSACTION_URL = '/api/explorer/v1/transactions'
const PER_PAGE = 10
const SERVICE_ID = 3
const TX_TRANSFER_ID = 0
const TX_ISSUE_ID = 1
const TX_WALLET_ID = 2
const Wallet = Exonum.newType(proto.exonum.examples.cryptocurrency_advanced.Wallet)

const transferTransaction = new Exonum.Transaction({
  serviceId: SERVICE_ID,
  methodId: TX_TRANSFER_ID,
  schema: proto.exonum.examples.cryptocurrency_advanced.Transfer
})

const issueTransaction = new Exonum.Transaction({
  schema: proto.exonum.examples.cryptocurrency_advanced.Issue,
  serviceId: SERVICE_ID,
  methodId: TX_ISSUE_ID
})

const walletTx = new Exonum.Transaction({
  schema: proto.exonum.examples.cryptocurrency_advanced.CreateWallet,
  serviceId: SERVICE_ID,
  methodId: TX_WALLET_ID
})

function deserializeWalletTx (transaction) {
  const txTypes = [transferTransaction, issueTransaction, walletTx]
  for (const tx of txTypes) {
    const txData = tx.deserialize(Exonum.hexadecimalToUint8Array(transaction))
    if (txData) {
      return Object.assign({}, txData.payload, {
        hash: txData.hash(),
        to: txData.payload.to ? Exonum.uint8ArrayToHexadecimal(txData.payload.to.data) : undefined
      })
    }
  }
  return { name: 'initialTx' }
}

module.exports = {
  install (Vue) {
    Vue.prototype.$blockchain = {
      generateKeyPair () {
        return Exonum.keyPair()
      },

      generateSeed () {
        return Exonum.randomUint64()
      },

      createWallet (keyPair, name) {
        const transaction = walletTx.create({ name }, keyPair).serialize()
        // Send transaction into blockchain
        return Exonum.send(TRANSACTION_URL, transaction)
      },

      addFunds (keyPair, amountToAdd, seed) {
        // Transaction data
        const data = {
          amount: amountToAdd.toString(),
          seed: seed
        }
        const transaction = issueTransaction.create(data, keyPair).serialize()

        // Send transaction into blockchain
        return Exonum.send(TRANSACTION_URL, transaction)
      },

      transfer (keyPair, receiver, amountToTransfer, seed) {
        // Transaction data
        const data = {
          to: { data: Exonum.hexadecimalToUint8Array(Exonum.publicKeyToAddress(receiver)) },
          amount: amountToTransfer,
          seed: seed
        }
        const transaction = transferTransaction.create(data, keyPair).serialize()

        // Send transaction into blockchain
        return Exonum.send(TRANSACTION_URL, transaction)
      },

      getWallet (publicKey) {
        return axios.get('/api/services/supervisor/consensus-config').then(response => {
          // actual list of public keys of validators
          const validators = response.data.validator_keys.map(validator => validator.consensus_key)

          return axios.get(`/api/services/crypto/v1/wallets/info?pub_key=${publicKey}`)
            .then(response => response.data)
            .then(({ block_proof, wallet_proof, wallet_history }) => {
              Exonum.verifyBlock(block_proof, validators)
              const tableRootHash = Exonum.verifyTable(wallet_proof.to_table, block_proof.block.state_hash, 'crypto.wallets')
              const walletProof = new Exonum.MapProof(wallet_proof.to_wallet, Exonum.MapProof.rawKey(Exonum.PublicKey), Wallet)
              if (walletProof.merkleRoot !== tableRootHash) throw new Error('Wallet proof is corrupted')

              const wallet = walletProof.entries.get(Exonum.publicKeyToAddress(publicKey))
              if (typeof wallet === undefined) throw new Error('Wallet not found')

              const verifiedTransactions = new Exonum.ListProof(wallet_history.proof, Exonum.Hash)
              const hexHistoryHash = Exonum.uint8ArrayToHexadecimal(new Uint8Array(wallet.history_hash.data))
              if (verifiedTransactions.merkleRoot !== hexHistoryHash) throw new Error('Transactions proof is corrupted')

              const validIndexes = verifiedTransactions
                .entries
                .every(({ index }, i) => i === index)
              if (!validIndexes) throw new Error('Invalid transaction indexes in the proof')

              const transactions = wallet_history.transactions.map(deserializeWalletTx)

              const correctHashes = transactions.every(({ hash }, i) => verifiedTransactions.entries[i].value === hash)
              if (!correctHashes) throw new Error('Transaction hash mismatch')

              return {
                block: block_proof.block,
                wallet: wallet,
                transactions: transactions
              }
            })
        })
      },

      getBlocks (latest) {
        const suffix = !isNaN(latest) ? '&latest=' + latest : ''
        return axios.get(`/api/explorer/v1/blocks?count=${PER_PAGE}${suffix}`).then(response => response.data)
      },

      getBlock (height) {
        return axios.get(`/api/explorer/v1/block?height=${height}`).then(response => response.data)
      },

      getTransaction (hash) {
        return axios.get(`/api/explorer/v1/transactions?hash=${hash}`)
          .then(response => response.data)
          .then(data => {
            data.content = deserializeWalletTx(data.message)
            return data
          })
      }
    }
  }
}

