import * as Exonum from 'exonum-client'
import axios from 'axios'

const PER_PAGE = 10
const ATTEMPTS = 10
const ATTEMPT_TIMEOUT = 500
const PROTOCOL_VERSION = 0
const SERVICE_ID = 130
const TX_ID = 0

const TableKey = Exonum.newType({
  fields: [
    {name: 'service_id', type: Exonum.Uint16},
    {name: 'table_index', type: Exonum.Uint16}
  ]
})
const SystemTime = Exonum.newType({
  fields: [
    { name: 'secs', type: Exonum.Uint64 },
    { name: 'nanos', type: Exonum.Uint32 }
  ]
})
const Timestamp = Exonum.newType({
  fields: [
    { name: 'content_hash', type: Exonum.Hash },
    { name: 'metadata', type: Exonum.String }
  ]
})
const TimestampEntry = Exonum.newType({
  fields: [
    { name: 'timestamp', type: Timestamp },
    { name: 'tx_hash', type: Exonum.Hash },
    { name: 'time', type: SystemTime }
  ]
})

function waitForAcceptance(response) {
  let attempt = ATTEMPTS

  if (response.data.debug) {
    throw new Error(response.data.description)
  }

  return (function makeAttempt() {
    return axios.get(`/api/explorer/v1/transactions/${response.data}`).then(response => {
      if (response.data.type === 'committed') {
        return response.data
      } else {
        if (--attempt > 0) {
          return new Promise((resolve) => {
            setTimeout(resolve, ATTEMPT_TIMEOUT)
          }).then(makeAttempt)
        } else {
          throw new Error('Transaction has not been found')
        }
      }
    })
  })()
}

module.exports = {
  install(Vue) {
    Vue.prototype.$blockchain = {
      createTimestamp: (hash, metadata) => {
        // Generate a new signing key pair
        const keyPair = Exonum.keyPair()

        // Describe transaction
        const TxTimestamp = Exonum.newMessage({
          protocol_version: PROTOCOL_VERSION,
          service_id: SERVICE_ID,
          message_id: TX_ID,
          fields: [
            { name: 'pub_key', type: Exonum.PublicKey },
            { name: 'content', type: Timestamp }
          ]
        })

        // Transaction data
        const data = {
          pub_key: keyPair.publicKey,
          content: {
            content_hash: hash,
            metadata: metadata
          }
        }

        // Sign transaction
        const signature = TxTimestamp.sign(keyPair.secretKey, data)

        // Send transaction into blockchain
        return axios.post('/api/services/timestamping/v1/timestamps', {
          protocol_version: PROTOCOL_VERSION,
          service_id: SERVICE_ID,
          message_id: TX_ID,
          body: data,
          signature: signature
        }).then(waitForAcceptance)
      },

      getTimestamp: hash => {
        return axios.get(`/api/services/timestamping/v1/timestamps/value/${hash}`).then(response => response.data)
      },

      getTimestampProof: hash => {
        return axios.get('/api/services/configuration/v1/configs/actual').then(response => {
          // actual list of public keys of validators
          const validators = response.data.config.validator_keys.map(validator => validator.consensus_key)

          return axios.get(`/api/services/timestamping/v1/timestamps/proof/${hash}`)
            .then(response => response.data)
            .then(data => {
              if (!Exonum.verifyBlock(data.block_info, validators)) {
                throw new Error('Block can not be verified')
              }

              // find root hash of table with all tables
              const tableKey = TableKey.hash({
                service_id: SERVICE_ID,
                table_index: 0
              })
              const stateProof = new Exonum.MapProof(data.state_proof, Exonum.Hash, Exonum.Hash)
              if (stateProof.merkleRoot !== data.block_info.block.state_hash) {
                throw new Error('State proof is corrupted')
              }
              const timestampsHash = stateProof.entries.get(tableKey)
              if (typeof timestampsHash === 'undefined') {
                throw new Error('Timestamps table not found')
              }

              // find timestamp in the tree of all timestamps
              const timestampProof = new Exonum.MapProof(data.timestamp_proof, Exonum.Hash, TimestampEntry)
              if (timestampProof.merkleRoot !== timestampsHash) {
                throw new Error('Timestamp proof is corrupted')
              }
              const timestamp = timestampProof.entries.get(hash)
              if (typeof timestamp === 'undefined') {
                throw new Error('Timestamp not found')
              }

              return timestamp
            })
        })
      },

      getBlocks(latest) {
        const suffix = !isNaN(latest) ? '&latest=' + latest : ''
        return axios.get(`/api/explorer/v1/blocks?count=${PER_PAGE}${suffix}`).then(response => response.data)
      },

      getBlock(height) {
        return axios.get(`/api/explorer/v1/blocks/${height}`).then(response => response.data)
      },

      getTransaction(hash) {
        return axios.get(`/api/explorer/v1/transactions/${hash}`).then(response => response.data)
      }
    }
  }
}
