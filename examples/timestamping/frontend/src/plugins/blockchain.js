
import * as Exonum from '../../exonum-client/dist/exonum-client'
// import * as Exonum from 'exonum-client'
import * as Protobuf from 'protobufjs/light'
import axios from 'axios'
import * as proto from '../../proto/protocol.js'

const PER_PAGE = 10;
const SERVICE_ID = 130;
const TX_ID = 0;
const TABLE_INDEX = 0;
const SystemTime = Exonum.newType({
  fields: [
    { name: 'secs', type: Exonum.Uint64 },
    { name: 'nanos', type: Exonum.Uint32 }
  ]
});
const Timestamp = Exonum.newType({
  fields: [
    { name: 'content_hash', type: Exonum.Hash },
    { name: 'metadata', type: Exonum.String }
  ]
});
const TimestampEntry = Exonum.newType({
  fields: [
    { name: 'timestamp', type: Timestamp },
    { name: 'tx_hash', type: Exonum.Hash },
    { name: 'time', type: SystemTime }
  ]
});

module.exports = {
  install(Vue) {
    Vue.prototype.$blockchain = {
      generateKeyPair() {
        return Exonum.keyPair()
      },

      createTimestamp: (keyPair, hash, metadata) => {
        // Define transaction data
        const data = {
          content: {
              contentHash: { data: Vue.prototype.$crypto.fromHexString(hash) },
              metadata: metadata
          }
        };


        // Define Exonum transaction
        const transaction = Exonum.newTransaction({
          author: keyPair.publicKey,
          service_id: SERVICE_ID,
          message_id: TX_ID,
          schema: proto.exonum.examples.timestamping.TxTimestamp
        });

        // Send transaction into blockchain
        return transaction.send('/api/explorer/v1/transactions', data, keyPair.secretKey)
      },

      getTimestamp: hash => {
        return axios.get(`/api/services/timestamping/v1/timestamps/value?hash=${hash}`).then(response => response.data)
      },

      getTimestampProof: hash => {
        return axios.get('/api/services/configuration/v1/configs/actual').then(response => {
          // Get actual list of public keys of validators
          const validators = response.data.config.validator_keys.map(validator => validator.consensus_key);

          return axios.get(`/api/services/timestamping/v1/timestamps/proof?hash=${hash}`)
            .then(response => response.data)
            .then(data => {
              if (!Exonum.verifyBlock(data.block_info, validators)) {
                throw new Error('Block can not be verified')
              }

              // verify table timestamps in the root tree
              const tableRootHash = Exonum.verifyTable(data.state_proof, data.block_info.block.state_hash, SERVICE_ID, TABLE_INDEX);

              // find timestamp in the tree of all timestamps
              const timestampProof = new Exonum.MapProof(data.timestamp_proof, Exonum.Hash, TimestampEntry);
              if (timestampProof.merkleRoot !== tableRootHash) {
                throw new Error('Timestamp proof is corrupted')
              }
              const timestamp = timestampProof.entries.get(hash);
              if (typeof timestamp === 'undefined') {
                throw new Error('Timestamp not found')
              }

              return timestamp
            })
        })
      },

      getBlocks(latest) {
        const suffix = !isNaN(latest) ? '&latest=' + latest : '';
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
};