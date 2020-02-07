import Vue from 'vue/dist/vue'
import axios from 'axios'
import MockAdapter from 'axios-mock-adapter'
import * as Blockchain from '../src/plugins/blockchain.js'
import actual from './data/actual.json'
import proof from './data/proof.json'
import 'babel-polyfill'

const mock = new MockAdapter(axios)
const bigIntRegex = /[0-9]+/i
const hexRegex = /[0-9A-Fa-f]+/i
const TRANSACTION_URL = '/api/explorer/v1/transactions'
const TRANSACTION_EXPLORER_URL = '/api/explorer/v1/transactions?hash='
const PROOF_URL = '/api/services/crypto/v1/wallets/info?pub_key='
const keyPair = {
  'publicKey': '68110bb9aa704d8fb8e711421e2a848ff7b72f910209af8723d7b45591b21bd7',
  'secretKey': 'd678892e2891dd54f9da7e94702e1d882c9f4a75b6e8e67dc40fe934fa6843aa68110bb9aa704d8fb8e711421e2a848ff7b72f910209af8723d7b45591b21bd7'
}

Vue.use(Blockchain)

// Mock `createWallet` transaction
const createWalletTxHash = '060ae9f8a22f53ea127a3486a7636756027ca1dfe06976b3565d54336ad93b4e'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '0a100a0e0a040803100212060a044976616e12220a2068110bb9aa704d8fb8e711421e2a848ff7b72f910209af8723d7b45591b21bd71a420a400c1f2260b4e1470529a66200471f57d863963647c2b0125acde0a5a381ad4363012b0f65d8c672f554f9b60edfff2be6f7de31fa42eaa9f8bb92855cb508f50b'
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${createWalletTxHash}`).replyOnce(200, { 'type': 'in-pool' })

mock.onGet(`${TRANSACTION_EXPLORER_URL}${createWalletTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock `addFunds` transaction
const addFundsTxHash = 'a6ae586be952539db0d76741e75e29ab22129c83889242ae6a8a3d7167880272'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '0a160a140a0408031001120c080a109081c18ada99e6b20112220a2068110bb9aa704d8fb8e711421e2a848ff7b72f910209af8723d7b45591b21bd71a420a40764404fcb6370874e41c6c69d41d4d6e2378ecefa9909fc4afe5d312f4170dd1bbb75d71d71b8c20b84c5e2699836e4f466c9f9eac06c9bb54730a15b9db9508'
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${addFundsTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock `transfer` transaction
const transferTxHash = '85e2c97aab7d2b6518850b3c9f647b1bb2fa7f8370f33c6f9b6c11cfa6371969'
mock.onPost(TRANSACTION_URL, {
  'tx_body': '78cf8b5e5c020696319eb32a1408e6c65e7d97733d34528fbdce08438a0243e80000800000000a220a20278663010ebe1136011618ad5be1b9d6f51edc5b6c6b51b5450ffc72f54a57df10191880a0db99c6b080bc6ba0bfeb12fc750df184136bd8d9a4f33676b8ee6e1e40754d7d19f0cb4f62db67e36e83253e737dce0ec3a6566857ef71de440d329fd470e77fed232d2411590c'
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}${transferTxHash}`).replyOnce(200, { 'type': 'committed' })

// Mock proof
mock.onGet('/api/services/supervisor/consensus-config').reply(200, actual)

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
    const amountToAdd = '10'
    const seed = '100654575627813010'

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

    expect(data.wallet)
      .toEqual({
          'owner': {
            'data': [
              16,
              50,
              110,
              218,
              148,
              241,
              6,
              249,
              181,
              172,
              96,
              154,
              25,
              67,
              132,
              255,
              239,
              225,
              189,
              72,
              92,
              192,
              63,
              191,
              221,
              77,
              244,
              203,
              153,
              148,
              109,
              195]
          },
          'name': 'Ivan',
          'balance': 240,
          'history_len': 7,
          'history_hash': {
            'data': [
              82,
              43,
              251,
              102,
              18,
              193,
              69,
              90,
              184,
              25,
              60,
              65,
              59,
              156,
              142,
              24,
              241,
              61,
              92,
              203,
              187,
              11,
              178,
              216,
              130,
              196,
              17,
              151,
              72,
              217,
              213,
              174]
          }
        }
      )
  })
})
