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

mock.onPost('/api/services/timestamping/v1/timestamps', {
  'protocol_version': 0,
  'service_id': 130,
  'message_id': 0,
  'signature': 'c79d59272f2734e65e2b657584025aa26512ba7d3784438f21ea8789c2dd3ffd2ca15be3b0cdff70e92f17bf8a8e9e9d2c582dcad64c46870bc09a4ef2491702',
  'body': {
    'pub_key': '0fe4d28f33b4c37ea2f6b433cc572f60d02b3f5b1638b0427dda2d7f5c028533',
    'content': {
      'content_hash': '966c80fec91149a85b2a496113aca0d9fefbc0edec6e4b2f8d0b24aaea9445f8',
      'metadata': 'Some contract'
    }
  }
}).reply(200)

mock.onGet('/api/explorer/v1/transactions?hash=069020ce9a066404b8c527558146ea05b072e986d3fd586a9790d9d89829fc72').replyOnce(200, txNotAccepted)

mock.onGet('/api/explorer/v1/transactions?hash=069020ce9a066404b8c527558146ea05b072e986d3fd586a9790d9d89829fc72').replyOnce(200, txAccepted)

mock.onGet('/api/services/configuration/v1/configs/actual').reply(200, actual)

mock.onGet('/api/services/timestamping/v1/timestamps/proof?hash=ce15c12c3d03d11f317acf503195b61853088f55b82b2495f243211927bc35d6').reply(200, proof)

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
      publicKey: '0fe4d28f33b4c37ea2f6b433cc572f60d02b3f5b1638b0427dda2d7f5c028533',
      secretKey: '1d061f0ed3fd21b6776b41c22d1e850621c647aa9b87b4e54b1564bc3e70e5480fe4d28f33b4c37ea2f6b433cc572f60d02b3f5b1638b0427dda2d7f5c028533'
    }
    const hash = '966c80fec91149a85b2a496113aca0d9fefbc0edec6e4b2f8d0b24aaea9445f8'
    const metadata = 'Some contract'

    await expect(Vue.prototype.$blockchain.createTimestamp(keyPair, hash, metadata)).resolves
  })

  it('should get timestamp proof and verify it', async () => {
    const hash = 'ce15c12c3d03d11f317acf503195b61853088f55b82b2495f243211927bc35d6'

    await expect(Vue.prototype.$blockchain.getTimestampProof(hash)).resolves.toEqual({
      'time': {
        'nanos': 6577000,
        'secs': '1531152169'
      },
      'timestamp': {
        'content_hash': 'ce15c12c3d03d11f317acf503195b61853088f55b82b2495f243211927bc35d6',
        'metadata': ''
      },
      'tx_hash': 'a5fa42cb59197fde244307227b9d177a49fe443a87a566beead2dc48577f149a'
    })
  })
})
