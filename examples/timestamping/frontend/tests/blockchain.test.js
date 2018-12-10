import Vue from 'vue/dist/vue'
import axios from 'axios'
import MockAdapter from 'axios-mock-adapter'
import * as Blockchain from '../src/plugins/blockchain.js'
import actual from './data/actual.json'
import txNotAccepted from './data/not-accepted.json'
import txAccepted from './data/accepted.json'
import proof from './data/proof.json'

const mock = new MockAdapter(axios)
const hexRegex = /[0-9A-Fa-f]+/i;

Vue.use(Blockchain)

mock.onPost('/api/explorer/v1/transactions', {
  tx_body: 'e613261b6a0a6283bf3180945a8b83421f98adf7f4ff07acd989eea430887b880000820000000a2a0a220a20d3677849f120ea4f1f7eea08a9efd8730373bde9a7e7676eb4126145c73c597a120474657374b98822feb0b4e020ab60d3b3d0e56c5086d8af01e3821cdc43ba5f90b81985a8a216715067d3198037823a4faaca7e1cd7ae30b1f40833cc67b79bb5cfb2600e'
}).reply(200)

mock.onGet('/api/explorer/v1/transactions?hash=05d621b1a62163e132e74430afe8d01ed873e68bb8e9f7abd9b7c72e1c7dbdc2').replyOnce(200, txNotAccepted)

mock.onGet('/api/explorer/v1/transactions?hash=05d621b1a62163e132e74430afe8d01ed873e68bb8e9f7abd9b7c72e1c7dbdc2').replyOnce(200, txAccepted)

mock.onGet('/api/services/configuration/v1/configs/actual').reply(200, actual)

mock.onGet('/api/services/timestamping/v1/timestamps/proof?hash=504d270ff8e300b8b48aeb1b61e7f96f6a369fc93321187d678c06b4ac5b8d4f').reply(200, proof)

describe('Interaction with blockchain', () => {
  
  it('should create new timestamp', async () => {
    const keyPair = {
      publicKey: 'e613261b6a0a6283bf3180945a8b83421f98adf7f4ff07acd989eea430887b88',
      secretKey: 'cf42162b483142e452aa743d9cb848afafee3d441c57c122c533e200570fc1b0e613261b6a0a6283bf3180945a8b83421f98adf7f4ff07acd989eea430887b88'
    }
    const hash = 'd3677849f120ea4f1f7eea08a9efd8730373bde9a7e7676eb4126145c73c597a'
    const metadata = 'test'

    await expect(Vue.prototype.$blockchain.createTimestamp(keyPair, hash, metadata)).resolves
  })

  /*
  it('should get timestamp proof and verify it', async () => {
    const hash = '504d270ff8e300b8b48aeb1b61e7f96f6a369fc93321187d678c06b4ac5b8d4f'

    await expect(Vue.prototype.$blockchain.getTimestampProof(hash)).resolves.toEqual({
      'time': {
        'nanos': 660266000,
        'secs': '1538746046'
      },
      'timestamp': {
        'content_hash': hash,
        'metadata': 'test'
      },
      'tx_hash': '05d621b1a62163e132e74430afe8d01ed873e68bb8e9f7abd9b7c72e1c7dbdc2'
    })
  })*/
})
