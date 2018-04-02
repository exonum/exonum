import * as Exonum from 'exonum-client'
import axios from 'axios'

const TX_URL = '/api/services/cryptocurrency/v1/wallets/transaction'
const PER_PAGE = 10

const ATTEMPTS = 10
const ATTEMPT_TIMEOUT = 500
const NETWORK_ID = 0
const PROTOCOL_VERSION = 0
const SERVICE_ID = 128
const TX_WALLET_ID = 130
const TX_ISSUE_ID = 129
const TX_TRANSFER_ID = 128

const TableKey = Exonum.newType({
  fields: [
    { name: 'service_id', type: Exonum.Uint16 },
    { name: 'table_index', type: Exonum.Uint16 }
  ]
})
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

function getTransaction(transactionId) {
  switch (transactionId) {
    case TX_WALLET_ID:
      return Exonum.newMessage({
        network_id: NETWORK_ID,
        protocol_version: PROTOCOL_VERSION,
        service_id: SERVICE_ID,
        message_id: TX_WALLET_ID,
        fields: [
          { name: 'pub_key', type: Exonum.PublicKey },
          { name: 'name', type: Exonum.String }
        ]
      })
    case TX_ISSUE_ID:
      return Exonum.newMessage({
        network_id: NETWORK_ID,
        protocol_version: PROTOCOL_VERSION,
        service_id: SERVICE_ID,
        message_id: TX_ISSUE_ID,
        fields: [
          { name: 'wallet', type: Exonum.PublicKey },
          { name: 'amount', type: Exonum.Uint64 },
          { name: 'seed', type: Exonum.Uint64 }
        ]
      })
    case TX_TRANSFER_ID:
      return Exonum.newMessage({
        network_id: NETWORK_ID,
        protocol_version: PROTOCOL_VERSION,
        service_id: SERVICE_ID,
        message_id: TX_TRANSFER_ID,
        fields: [
          { name: 'from', type: Exonum.PublicKey },
          { name: 'to', type: Exonum.PublicKey },
          { name: 'amount', type: Exonum.Uint64 },
          { name: 'seed', type: Exonum.Uint64 }
        ]
      })
    default:
      throw new Error('Unknown transaction ID has been passed')
  }
}

function getPublicKeyOfTransaction(transactionId, transaction) {
  switch (transactionId) {
    case TX_TRANSFER_ID:
      return transaction.from
    case TX_ISSUE_ID:
      return transaction.wallet
    case TX_WALLET_ID:
      return transaction.pub_key
    default:
      throw new Error('Unknown transaction ID has been passed')
  }
}

function getWallet(keyPair) {
  return axios.get('/api/services/configuration/v1/configs/actual').then(response => {
    // actual list of public keys of validators
    const validators = response.data.config.validator_keys.map(validator => {
      return validator.consensus_key
    })

    return axios.get('/api/services/cryptocurrency/v1/wallets/info?pubkey=' + keyPair.publicKey).then(response => {
      return response.data
    }).then((data) => {
      if (!Exonum.verifyBlock(data.block_info, validators, NETWORK_ID)) {
        throw new Error('Block can not be verified')
      }

      // find root hash of table with wallets in the tree of all tables
      const tableKey = TableKey.hash({
        service_id: SERVICE_ID,
        table_index: 0
      })
      const walletsHash = Exonum.merklePatriciaProof(data.block_info.block.state_hash, data.wallet.mpt_proof, tableKey)
      if (walletsHash === null) {
        throw new Error('Wallets table not found')
      }

      // find wallet in the tree of all wallets
      const wallet = Exonum.merklePatriciaProof(walletsHash, data.wallet.value, keyPair.publicKey, Wallet)
      if (wallet === null) {
        throw new Error('Wallet not found')
      }

      // get transactions
      const transactionsMetaData = Exonum.merkleProof(
        wallet.history_hash,
        wallet.history_len,
        data.wallet_history.mt_proof,
        [0, wallet.history_len],
        TransactionMetaData
      )

      if (data.wallet_history.values.length !== transactionsMetaData.length) {
        // number of transactions in wallet history is not equal
        // to number of transactions in array with transactions meta data
        throw new Error('Transactions can not be verified')
      }

      // validate each transaction
      let transactions = []
      for (let i = 0; i < data.wallet_history.values.length; i++) {
        let Transaction = getTransaction(data.wallet_history.values[i].message_id)
        const publicKey = getPublicKeyOfTransaction(data.wallet_history.values[i].message_id, data.wallet_history.values[i].body)

        Transaction.signature = data.wallet_history.values[i].signature

        if (Transaction.hash(data.wallet_history.values[i].body) !== transactionsMetaData[i].tx_hash) {
          throw new Error('Invalid transaction hash has been found')
        } else if (!Transaction.verifySignature(data.wallet_history.values[i].signature, publicKey, data.wallet_history.values[i].body)) {
          throw new Error('Invalid transaction signature has been found')
        }

        transactions.push(Object.assign({
          hash: transactionsMetaData[i].tx_hash,
          status: transactionsMetaData[i].execution_status
        }, data.wallet_history.values[i]))
      }

      return {
        block: data.block_info.block,
        wallet: wallet,
        transactions: transactions
      }
    })
  })
}

function waitForAcceptance(keyPair, hash) {
  let attempt = ATTEMPTS

  return (function makeAttempt() {
    return getWallet(keyPair).then(data => {
      // find transaction in a wallet proof
      if (typeof data.transactions.find((transaction) => transaction.hash === hash) === 'undefined') {
        if (--attempt > 0) {
          return new Promise((resolve) => {
            setTimeout(resolve, ATTEMPT_TIMEOUT)
          }).then(makeAttempt)
        } else {
          throw new Error('Transaction has not been found')
        }
      } else {
        return data
      }
    })
  })()
}

module.exports = {
  install(Vue) {
    Vue.prototype.$blockchain = {
      createWallet: name => {
        const keyPair = Exonum.keyPair()

        const TxCreateWallet = getTransaction(TX_WALLET_ID)

        const data = {
          pub_key: keyPair.publicKey,
          name: name
        }

        const signature = TxCreateWallet.sign(keyPair.secretKey, data)

        return axios.post(TX_URL, {
          network_id: NETWORK_ID,
          protocol_version: PROTOCOL_VERSION,
          service_id: SERVICE_ID,
          message_id: TX_WALLET_ID,
          signature: signature,
          body: data
        }).then(() => keyPair)
      },

      addFunds: (keyPair, amountToAdd) => {
        const TxIssue = getTransaction(TX_ISSUE_ID)

        const data = {
          wallet: keyPair.publicKey,
          amount: amountToAdd.toString(),
          seed: Exonum.randomUint64()
        }

        const signature = TxIssue.sign(keyPair.secretKey, data)

        return axios.post(TX_URL, {
          network_id: NETWORK_ID,
          protocol_version: PROTOCOL_VERSION,
          service_id: SERVICE_ID,
          message_id: TX_ISSUE_ID,
          signature: signature,
          body: data
        }).then(response => waitForAcceptance(keyPair, response.data.tx_hash))
      },

      transfer: (keyPair, receiver, amountToTransfer) => {
        const TxTransfer = getTransaction(TX_TRANSFER_ID)

        const data = {
          from: keyPair.publicKey,
          to: receiver,
          amount: amountToTransfer,
          seed: Exonum.randomUint64()
        }

        const signature = TxTransfer.sign(keyPair.secretKey, data)

        return axios.post(TX_URL, {
          network_id: NETWORK_ID,
          protocol_version: PROTOCOL_VERSION,
          service_id: SERVICE_ID,
          message_id: TX_TRANSFER_ID,
          signature: signature,
          body: data
        }).then(response => waitForAcceptance(keyPair, response.data.tx_hash))
      },

      getWallet: getWallet,

      getBlocks: latest => {
        const suffix = !isNaN(latest) ? '&latest=' + latest : ''
        return axios.get(`/api/explorer/v1/blocks?count=${PER_PAGE}` + suffix).then(response => response.data)
      },

      getBlock: height => {
        return axios.get(`/api/explorer/v1/blocks/${height}`).then(response => response.data)
      },

      getTransaction: hash => {
        return axios.get(`/api/system/v1/transactions/${hash}`).then(response => response.data)
      }
    }
  }
}
