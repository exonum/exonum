import Vue from 'vue/dist/vue'
import axios from 'axios'
import MockAdapter from 'axios-mock-adapter'
import * as Blockchain from '../src/plugins/blockchain.js'
import actual from './data/actual.json'
import proof from './data/proof.json'

const mock = new MockAdapter(axios)
const bigIntRegex = /[0-9]+/i;
const hexRegex = /[0-9A-Fa-f]+/i;
const TRANSACTION_URL = '/api/explorer/v1/transactions'
const TRANSACTION_EXPLORER_URL = '/api/explorer/v1/transactions?hash='
const PROOF_URL = '/api/services/cryptocurrency/v1/wallets/info?pub_key='
const keyPair = {
  publicKey: '78cf8b5e5c020696319eb32a1408e6c65e7d97733d34528fbdce08438a0243e8',
  secretKey: 'b5b3ccf6ca4475b7ff3d910d5ab31e4723098490a3e341dd9d2896b42ebc9f8978cf8b5e5c020696319eb32a1408e6c65e7d97733d34528fbdce08438a0243e8'
}

Vue.use(Blockchain)

// Mock `createWallet` transaction
const createWalletTxHash = '55209b3c6bd8593b9c90eacd3a57cfc448ebc0d47316235a4ca3a1751548a384'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '78cf8b5e5c020696319eb32a1408e6c65e7d97733d34528fbdce08438a0243e80000800002000a084a6f686e20446f65a71558ef2f2d592acfffbac71ea13327a78be83d6977240d3ca8cf4a92ba3d87cd30b9df1ca9b83147be274a85369ce5a2ce3e3a490be6acbaca48c764b40907'
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${createWalletTxHash}`).replyOnce(200, { 'type': 'in-pool' })

mock.onGet(`${TRANSACTION_EXPLORER_URL}${createWalletTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock `addFunds` transaction
const addFundsTxHash = 'b26f1e9e01a6f7f07d6224597992bb04fd5a4bd633faf0a28c384fa2b99ba322'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '78cf8b5e5c020696319eb32a1408e6c65e7d97733d34528fbdce08438a0243e800008000010008321080d0b6db99b1c3f1890106ecdedffe9d00b6c1911e7a75f8c0fea17554f31497c914686bc63ad175cabfb02eaa40230573bb1ff1c4d98cd996c9c7c0eb54843f306d03ae4bf24aa72408'
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${addFundsTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock `transfer` transaction
const transferTxHash = '85e2c97aab7d2b6518850b3c9f647b1bb2fa7f8370f33c6f9b6c11cfa6371969'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '78cf8b5e5c020696319eb32a1408e6c65e7d97733d34528fbdce08438a0243e80000800000000a220a20278663010ebe1136011618ad5be1b9d6f51edc5b6c6b51b5450ffc72f54a57df10191880a0db99c6b080bc6ba0bfeb12fc750df184136bd8d9a4f33676b8ee6e1e40754d7d19f0cb4f62db67e36e83253e737dce0ec3a6566857ef71de440d329fd470e77fed232d2411590c'
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
    const seed = '9935800087578782468'

    await expect(Vue.prototype.$blockchain.addFunds(keyPair, amountToAdd, seed)).resolves
  })

  it('should transfer funds', async () => {
    const receiver = '278663010ebe1136011618ad5be1b9d6f51edc5b6c6b51b5450ffc72f54a57df'
    const amountToTransfer = '25'
    const seed = '7743941227375415562'

    await expect(Vue.prototype.$blockchain.transfer(keyPair, receiver, amountToTransfer, seed)).resolves
  })

  it('should get wallet proof and verify it', async () => {
    const data = await Vue.prototype.$blockchain.getWallet(keyPair.publicKey)

    expect(data.wallet).toEqual({
      "pub_key": {
        "data": [120, 207, 139, 94, 92, 2, 6, 150, 49, 158, 179, 42, 20, 8, 230, 198, 94, 125, 151, 115, 61, 52, 82, 143, 189, 206, 8, 67, 138, 2, 67, 232]
      },
      "name": "John Doe",
      "balance": 100,
      "history_len": 1,
      "history_hash": {
        "data": [85, 32, 155, 60, 107, 216, 89, 59, 156, 144, 234, 205, 58, 87, 207, 196, 72, 235, 192, 212, 115, 22, 35, 90, 76, 163, 161, 117, 21, 72, 163, 132]
      }
    })
  })
})
