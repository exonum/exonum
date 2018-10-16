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
const createWalletTxHash = 'f7248195b3d0e2ca5c018a7cd26159d44c2c7e27a9e1f9150a1779d4f7f2420d'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '24cf3dd648b98abd9f76b427bcf32c2db4c509efa323c07dfbdad54b2bb9e87b00008000020008000000080000004a6f686e20446f65470c0d186a70e22c1d7d62e92de644b8789172058a7dfff92655c050a8853d1000be72dfb2fe9f941ce705c61b9da2a476167e0ee2c7d91d265da28361132703'
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${createWalletTxHash}`).replyOnce(200, { 'type': 'in-pool' })

mock.onGet(`${TRANSACTION_EXPLORER_URL}${createWalletTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock `addFunds` transaction
const addFundsTxHash = 'c8f0dbbfdf27f9118d4eb17eb0dbd0e4f3ecc5d62a69eb62d883e6f934750d7b'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '24cf3dd648b98abd9f76b427bcf32c2db4c509efa323c07dfbdad54b2bb9e87b0000800001003200000000000000fb586a6206b7c25c7dc31bf63c8476bccf7b43f2b3c7521db5b7c7f13300d011309b91055ec1c2e8fe4d82b3944f67918884dd51fdc43e7ce15f366625a85d30d72f3a27f74ccd01'
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${addFundsTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock `transfer` transaction
const transferTxHash = '2a9c7c66ef597bae9c1758e75534c88c8fa9cf4a72977cda80d12f18feaf9020'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '24cf3dd648b98abd9f76b427bcf32c2db4c509efa323c07dfbdad54b2bb9e87b0000800000008740e2dbe13dfe028e4c4afe27bb3d732f1c45977fa523fcce0d3a90f7ca5c0a05000000000000000093ab9928d470e7f30500dfa46b886e42f1cdcfb18675c90213898c88da4ec05ed40bb170181c9c57b241abd45166f63858805ceda90827a3b1462718117b7150a6b39a9d9fd40c'
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${transferTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock proof
mock.onGet('/api/services/configuration/v1/configs/actual').reply(200, actual)

mock.onGet(`${PROOF_URL}${keyPair.publicKey}`).replyOnce(200, proof)

describe('Interaction with blockchain', () => {
  it('should generate new signing key pair', () => {
    const keyPair = Vue.prototype.$blockchain.generateKeyPair()

    expect(keyPair.publicKey).toMatch(hexRegex)
    expect(keyPair.publicKey).toHaveLength(64)
    expect(keyPair.secretKey).toMatch(hexRegex)
    expect(keyPair.secretKey).toHaveLength(128)
  })

  it('should generate new random seed', () => {
    const seed = Vue.prototype.$blockchain.generateSeed()

    expect(seed).toMatch(bigIntRegex)
  })

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
  })
})
