import * as Exonum from 'exonum-client'
import axios from 'axios'
import * as proto from '../../proto/stubs.js'

const PER_PAGE = 10
const SERVICE_ID = 4
const TX_ID = 0
const TimestampEntry = Exonum.newType(proto.exonum.examples.timestamping.TimestampEntry)

module.exports = {
  install (Vue) {
    Vue.prototype.$blockchain = {
      generateKeyPair () {
        return Exonum.keyPair()
      },

      createTimestamp: (keyPair, hash, metadata) => {
        // Describe transaction
        const transaction = new Exonum.Transaction({
          serviceId: SERVICE_ID,
          methodId: TX_ID,
          schema: proto.exonum.examples.timestamping.Timestamp
        })

        // Transaction data
        const data = {
          content_hash: { data: Exonum.hexadecimalToUint8Array(hash) },
          metadata
        }

        // Send transaction into blockchain
        return Exonum.send('/api/explorer/v1/transactions', transaction.create(data, keyPair).serialize())
      },

      getTimestamp: hash => {
        return axios.get(`/api/services/timestamping/v1/timestamps/value?hash=${hash}`).then(response => response.data)
      },

      getTimestampProof: hash => {
        return axios.get('/api/services/supervisor/consensus-config').then(response => {
          // actual list of public keys of validators
          const validators = response.data.validator_keys.map(validator => validator.consensus_key)

          return axios.get(`/api/services/timestamping/v1/timestamps/proof?hash=${hash}`)
            .then(response => response.data)
            .then(({ timestamp_proof, state_proof, block_info }) => {
              Exonum.verifyBlock(block_info, validators)
              const tableRootHash = Exonum.verifyTable(state_proof, block_info.block.state_hash, 'timestamping.timestamps')
              const timestampProof = new Exonum.MapProof(timestamp_proof, Exonum.MapProof.rawKey(Exonum.PublicKey), TimestampEntry)
              if (timestampProof.merkleRoot !== tableRootHash) throw new Error('Timestamp proof is corrupted')

              const timestampEntry = timestampProof.entries.get(hash)
              if (typeof timestampEntry === 'undefined') throw new Error('Timestamp not found')

              return {
                timestamp: {
                  content_hash: Exonum.uint8ArrayToHexadecimal(new Uint8Array(timestampEntry.timestamp.content_hash.data)),
                  metadata: timestampEntry.timestamp.metadata
                },
                tx_hash: Exonum.uint8ArrayToHexadecimal(new Uint8Array(timestampEntry.tx_hash.data)),
                time: timestampEntry.time
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
        return axios.get(`/api/explorer/v1/transactions?hash=${hash}`).then(response => response.data)
      }
    }
  }
}
