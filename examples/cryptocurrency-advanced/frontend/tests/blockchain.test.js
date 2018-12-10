import Vue from 'vue/dist/vue'
import axios from 'axios'
import MockAdapter from 'axios-mock-adapter'
import * as Blockchain from '../src/plugins/blockchain.js'
import actual from './data/actual.json'
import proof from './data/proof.json'

const mock = new MockAdapter(axios)
const bigIntRegex = /[0-9]+/i;
const hexRegex = /[0-9A-Fa-f]+/i;
const TRANSACTION_URL = '/api/services/cryptocurrency/v1/wallets/transaction'
const TRANSACTION_EXPLORER_URL = '/api/explorer/v1/transactions?hash='
const PROOF_URL = '/api/services/cryptocurrency/v1/wallets/info?pub_key='
const keyPair = {
  publicKey: '24cf3dd648b98abd9f76b427bcf32c2db4c509efa323c07dfbdad54b2bb9e87b',
  secretKey: 'b47dcd8ac4598bf884ebfd178d40ed500ce31eaa5c5be495f532f0a3776db72f24cf3dd648b98abd9f76b427bcf32c2db4c509efa323c07dfbdad54b2bb9e87b'
}

Vue.use(Blockchain)

// Mock `createWallet` transaction
const createWalletTxHash = 'c0649d252aed811b185d0868534dc11d86017c6d277ed87fdfb8d8a87df463d8'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '24cf3dd648b98abd9f76b427bcf32c2db4c509efa323c07dfbdad54b2bb9e87b0000800002000a084a6f686e20446f65987e239da64e6bf40e8302f677c52f5ae39b928bdac17aaba516c6f492a0fef2bdd3cada6d62fea7f914c60c06eef3dc84eba88830bf0c05a688747447f3fb0b'
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${createWalletTxHash}`).replyOnce(200, { 'type': 'in-pool' })

mock.onGet(`${TRANSACTION_EXPLORER_URL}${createWalletTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock `addFunds` transaction
const addFundsTxHash = '3b703d5c8b3e27731c21ca73ed8afa8cf36ee4dce5d74a34d36845a4491b8a8e'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '24cf3dd648b98abd9f76b427bcf32c2db4c509efa323c07dfbdad54b2bb9e87b000080000100083210fbb1a993e6e0ade15cd8a180512e0e1a962e8092963f0882c570ed45b5a5058d4d72fff5ca6bbbde077493533d5898f9a2dedc4d4cbe4160f9ee0bed1f5a9089008566de1df7ce6f08'
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${addFundsTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock `transfer` transaction
const transferTxHash = '8d0349f704064a7ac381164308cf641958fdb1a59a635d2862a7c9327eafa611'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '24cf3dd648b98abd9f76b427bcf32c2db4c509efa323c07dfbdad54b2bb9e87b0000800000000a220a208740e2dbe13dfe028e4c4afe27bb3d732f1c45977fa523fcce0d3a90f7ca5c0a10051880a6aecd8985b5b8e7015a52de477b336b990c77b326d58a59bdf05c1782edf4f0d4f59940bd2b71b58580fb62f35faa45cc0c6d8966f24048632f5e49f45099c6f0002409066e5ace0a '
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${transferTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock proof
mock.onGet('/api/services/configuration/v1/configs/actual').reply(200, actual)

mock.onGet(`${PROOF_URL}${keyPair.publicKey}`).replyOnce(200, proof)

describe('Interaction with blockchain', () => {

  it('should generate new random seed', () => {
    const seed = Vue.prototype.$blockchain.generateSeed()

    expect(seed).toMatch(bigIntRegex)
  })
  /*

  it('should create new wallet', async () => {
    const name = 'John Doe'

    await expect(Vue.prototype.$blockchain.createWallet(keyPair, name)).resolves
  })

  it('should add funds', async () => {
    const amountToAdd = '50'
    const seed = '6684106035020060923'

    await expect(Vue.prototype.$blockchain.addFunds(keyPair, amountToAdd, seed)).resolves
  })

  it('should transfer funds', async () => {
    const receiver = '8740e2dbe13dfe028e4c4afe27bb3d732f1c45977fa523fcce0d3a90f7ca5c0a'
    const amountToTransfer = '5'
    const seed = '16677062690994885376'

    await expect(Vue.prototype.$blockchain.transfer(keyPair, receiver, amountToTransfer, seed)).resolves
  })

  it('should get wallet proof and verify it', async () => {
    const data = await Vue.prototype.$blockchain.getWallet(keyPair.publicKey)

    expect(data.wallet).toEqual({
      'balance': '145',
      'history_hash': '8bb3dd4745085255a18e44dcaace6f27014524398e162985fdea10b4fd91a6c2',
      'history_len': '3',
      'name': 'John Doe',
      'pub_key': '24cf3dd648b98abd9f76b427bcf32c2db4c509efa323c07dfbdad54b2bb9e87b'
    })
  })*/
})
