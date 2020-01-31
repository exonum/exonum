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

function TransferTransaction () {
  return new Exonum.Transaction({
    serviceId: SERVICE_ID,
    methodId: TX_TRANSFER_ID,
    schema: proto.exonum.examples.cryptocurrency_advanced.Transfer
  })
}

function IssueTransaction () {
  return new Exonum.Transaction({
    schema: proto.exonum.examples.cryptocurrency_advanced.Issue,
    serviceId: SERVICE_ID,
    methodId: TX_ISSUE_ID
  })
}

function WalletTx () {
  return new Exonum.Transaction({
    schema: proto.exonum.examples.cryptocurrency_advanced.CreateWallet,
    serviceId: SERVICE_ID,
    methodId: TX_WALLET_ID
  })
}

function deserializeWalletTx (transaction) {
  const txTypes = [TransferTransaction(), IssueTransaction()]
  for (const tx of txTypes) {
    const txData = tx.deserialize(Exonum.hexadecimalToUint8Array(transaction))
    if (txData) return Object.assign({}, txData.payload, {
      hash: txData.hash(),
      to: txData.payload.to ? Exonum.uint8ArrayToHexadecimal(txData.payload.to.data) : undefined
    })
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
        const walletTx = WalletTx()
        const transaction = walletTx.create({ name }, keyPair).serialize()
        // Send transaction into blockchain
        return Exonum.send(TRANSACTION_URL, transaction)
      },

      addFunds (keyPair, amountToAdd, seed) {
        // Describe transaction
        const issueTx = IssueTransaction()

        // Transaction data
        const data = {
          amount: amountToAdd.toString(),
          seed: seed
        }
        const transaction = issueTx.create(data, keyPair).serialize()

        // Send transaction into blockchain
        return Exonum.send(TRANSACTION_URL, transaction)
      },

      transfer (keyPair, receiver, amountToTransfer, seed) {
        // Describe transaction
        const transferTx = TransferTransaction()

        // Transaction data
        const data = {
          to: { data: Exonum.hexadecimalToUint8Array(Exonum.publicKeyToAddress(receiver)) },
          amount: amountToTransfer,
          seed: seed
        }
        console.log(data)
        const transaction = transferTx.create(data, keyPair).serialize()

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
              if (Exonum.verifyBlock(block_proof, validators) !== undefined) throw new Error('Block is invalid')
              const tableRootHash = Exonum.verifyTable(wallet_proof.to_table, block_proof.block.state_hash, 'crypto.wallets')
              const walletProof = new Exonum.MapProof(wallet_proof.to_wallet, Exonum.MapProof.rawKey(Exonum.PublicKey), Wallet)
              if (walletProof.merkleRoot !== tableRootHash) throw new Error('Wallet proof is corrupted')

              const wallet = walletProof.entries.get(Exonum.publicKeyToAddress(publicKey))
              if (typeof wallet === undefined) throw new Error('Wallet not found')

              const transactions = wallet_history.transactions.map(deserializeWalletTx)

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
