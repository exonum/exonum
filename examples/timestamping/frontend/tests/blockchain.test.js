import Vue from 'vue/dist/vue'
import axios from 'axios'
import MockAdapter from 'axios-mock-adapter'
import * as Blockchain from '../src/plugins/blockchain.js'
import actual from './data/actual.json'
import proof from './data/proof.json'
import 'babel-polyfill';

const mock = new MockAdapter(axios)
const hexRegex = /[0-9A-Fa-f]+/i

Vue.use(Blockchain)

const hash = '4ee9e65a836e525042b22f8270f68b4e1fbd18ac969cd12dea24093380031eae'
mock.onGet('/api/services/supervisor/consensus-config').reply(200, actual)
mock.onGet(`/api/services/timestamping/v1/timestamps/proof?hash=${hash}`).reply(200, proof)

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
    const data = await Vue.prototype.$blockchain.getTimestampProof(hash)

    await expect(Vue.prototype.$blockchain.getTimestampProof(hash)).resolves.toEqual({
        timestamp: {
          content_hash: '4ee9e65a836e525042b22f8270f68b4e1fbd18ac969cd12dea24093380031eae',
          metadata: 'hello world'
        },
        tx_hash: 'e5643f7a630573786fecc536a289b1c9689c9fe85e924f1cadb3115d5377d350',
        time: { seconds: 1581090568, nanos: 112863000 }
      }
    )
  })
})
