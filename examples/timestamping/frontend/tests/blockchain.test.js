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
  tx_body: '727b3af198eccbc76f4b942640f1f32c67731f1dd410859f191956ea3789b628000082000000080000002c000000504d270ff8e300b8b48aeb1b61e7f96f6a369fc93321187d678c06b4ac5b8d4f2800000004000000746573747ca84d8ddf5c7f1efcb3f96f6710208c4b590fc3fd6b6396d3fa9bc7dad4ade40e3b7b793b00e5311de945143da8c2537c946c6ada9eccfa0d01f4f185caaa04'
}).reply(200)

mock.onGet('/api/explorer/v1/transactions?hash=05d621b1a62163e132e74430afe8d01ed873e68bb8e9f7abd9b7c72e1c7dbdc2').replyOnce(200, txNotAccepted)

mock.onGet('/api/explorer/v1/transactions?hash=05d621b1a62163e132e74430afe8d01ed873e68bb8e9f7abd9b7c72e1c7dbdc2').replyOnce(200, txAccepted)

mock.onGet('/api/services/configuration/v1/configs/actual').reply(200, actual)

mock.onGet('/api/services/timestamping/v1/timestamps/proof?hash=504d270ff8e300b8b48aeb1b61e7f96f6a369fc93321187d678c06b4ac5b8d4f').reply(200, proof)

describe('Interaction with blockchain', () => {
  it('should generate new signing key pair', () => {
    const keyPair = Vue.prototype.$blockchain.generateKeyPair()

    expect(keyPair.publicKey).toMatch(hexRegex)
    expect(keyPair.publicKey).toHaveLength(64)
    expect(keyPair.secretKey).toMatch(hexRegex)
    expect(keyPair.secretKey).toHaveLength(128)
  })

  it('should create new timestamp', async () => {
    const keyPair = {
      publicKey: '727b3af198eccbc76f4b942640f1f32c67731f1dd410859f191956ea3789b628',
      secretKey: '658f5106aecfa46129ada041dd080d7ae3007f78300cdc1411f5bb11cda0bfab727b3af198eccbc76f4b942640f1f32c67731f1dd410859f191956ea3789b628'
    }
    const hash = '504d270ff8e300b8b48aeb1b61e7f96f6a369fc93321187d678c06b4ac5b8d4f'
    const metadata = 'test'

    await expect(Vue.prototype.$blockchain.createTimestamp(keyPair, hash, metadata)).resolves
  })

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
  })
})
