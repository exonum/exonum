import * as Exonum from 'exonum-client'
import bigInt from 'big-integer'
import axios from 'axios'

const PROTOCOL_VERSION = 0
const SERVICE_ID = 130
const TX_ID = 0

const ATTEMPTS = 10
const ATTEMPT_TIMEOUT = 500

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

function getSystemTime() {
  const now = Date.now()
  const secs = bigInt(now).over(1000)
  const nanos = bigInt(now).minus(secs.multiply(1000)).multiply(1000000)

  return {
    secs: secs.toString(),
    nanos: nanos.valueOf()
  }
}

function waitForAcceptance(hash) {
  let attempt = ATTEMPTS

  return (function makeAttempt() {
    return axios.get(`/api/explorer/v1/transactions/${hash}`).then(response => {
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
        }).then(response => waitForAcceptance(response.data.tx_hash)).then(() => keyPair)
      },

      getTimestamp: hash => {
        return axios.get(`/api/services/timestamping/v1/timestamps/value/${hash}`).then(response => response.data)
      },

      getTimestampProof: hash => {
        return axios.get(`/api/services/timestamping/v1/timestamps/proof/${hash}`).then(response => response.data)
      }
    }
  }
}
