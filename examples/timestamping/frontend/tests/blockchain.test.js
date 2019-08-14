import Vue from 'vue/dist/vue'
import axios from 'axios'
import MockAdapter from 'axios-mock-adapter'
import * as Blockchain from '../src/plugins/blockchain.js'
import actual from './data/actual.json'
import txNotAccepted from './data/not-accepted.json'
import txAccepted from './data/accepted.json'
import proof from './data/proof.json'
import 'babel-polyfill';

const mock = new MockAdapter(axios)
const hexRegex = /[0-9A-Fa-f]+/i

Vue.use(Blockchain)

mock.onPost('/api/explorer/v1/transactions', {
  tx_body: '25f7aad8046db3ce0cbbb8aa5444383305b4e2003fae578f57243a926148c5ae0000820000000a330a220a200cf4fd1634bcb6e0cfc0ae0e111931747c81f613534a66be8f8ec0eb0ed1acf9120d536f6d6520636f6e74726163740ad40aa875ae16524bd2ba51acbd804776e174efcb934dfb602aa9981a6361883c57571de141f8028e8f009c23f6567b985064c5e5b42a64c613ded2b1a89a0d'
}).reply(200)

mock.onGet('/api/explorer/v1/transactions?hash=ce0743cff6bdef0afa2f3af68b77612aa8f22190d3e94534268e3b784cfd7805').replyOnce(200, txNotAccepted)

mock.onGet('/api/explorer/v1/transactions?hash=ce0743cff6bdef0afa2f3af68b77612aa8f22190d3e94534268e3b784cfd7805').replyOnce(200, txAccepted)

mock.onGet('/api/services/configuration/v1/configs/actual').reply(200, actual)

mock.onGet('/api/services/timestamping/v1/timestamps/proof?hash=67fa96da3465c77befabd75b53027e39b35c79d0ed69a175fff5141516353cc3').reply(200, proof)

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
      publicKey: '25f7aad8046db3ce0cbbb8aa5444383305b4e2003fae578f57243a926148c5ae',
      secretKey: '16d47606dedbfec4cd44cea14cb43b2fdb2e0abe5f5c9c6e594a33b0e5bdf59125f7aad8046db3ce0cbbb8aa5444383305b4e2003fae578f57243a926148c5ae'
    }
    const hash = '0cf4fd1634bcb6e0cfc0ae0e111931747c81f613534a66be8f8ec0eb0ed1acf9'
    const metadata = 'Some contract'

    await expect(Vue.prototype.$blockchain.createTimestamp(keyPair, hash, metadata)).resolves
  })

  it('should get timestamp proof and verify it', async () => {
    const hash = '67fa96da3465c77befabd75b53027e39b35c79d0ed69a175fff5141516353cc3'

    await expect(Vue.prototype.$blockchain.getTimestampProof(hash)).resolves.toEqual({
      'time': {
        'nanos': 93835000,
        'seconds': 1565701488
      },
      'timestamp': {
        'content_hash': '67fa96da3465c77befabd75b53027e39b35c79d0ed69a175fff5141516353cc3',
        'metadata': 'Test data'
      },
      'tx_hash': '787f027f0f3e0e677f0de15163a7bcd4aacec49d960c5dac4fd31b3edb1a9092'
    })
  })
})
