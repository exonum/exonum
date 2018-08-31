import Vue from 'vue/dist/vue'
import axios from 'axios'
import MockAdapter from 'axios-mock-adapter'
import * as Blockchain from '../src/plugins/blockchain.js'
import actual from './data/actual.json'
import walletProof from './data/proof.json'
import addFundsTxNotAccepted from './data/add-funds-not-accepted.json'
import addFundsTxAccepted from './data/add-funds-accepted.json'
import transferTxNotAccepted from './data/transfer-not-accepted.json'
import transferTxAccepted from './data/transfer-accepted.json'

const mock = new MockAdapter(axios)
const bigIntRegex = /[0-9]+/i;
const hexRegex = /[0-9A-Fa-f]+/i;
const TRANSACTION_URL = '/api/services/cryptocurrency/v1/wallets/transaction'
const TRANSACTION_EXPLORER_URL = '/api/explorer/v1/transactions?hash='
const PROOF_URL = '/api/services/cryptocurrency/v1/wallets/info?pub_key='

Vue.use(Blockchain)

// Mock wallet proof loading
mock.onGet('/api/services/configuration/v1/configs/actual').reply(200, actual)

mock.onGet(`${PROOF_URL}ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600`).replyOnce(200, walletProof)

// Mock `createWallet` transaction
mock.onPost(TRANSACTION_URL, {
  'protocol_version': 0,
  'service_id': 128,
  'message_id': 2,
  'signature': 'c28cc03f7b2bb41cac2c83896be31a293594100e8fdced2d439165f7e9227b271b464408c694df887548971db70b2cf4f287f0781f1a0e9dfe9bfa0f0b80b70a',
  'body': {
    'pub_key': 'ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600',
    'name': 'John Doe'
  }
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}473fab385340f46b1e89f03124d05d849d7948ef11bc20ae569c96a15052704c`).replyOnce(200, {
  'type': 'in-pool'
})

mock.onGet(`${TRANSACTION_EXPLORER_URL}473fab385340f46b1e89f03124d05d849d7948ef11bc20ae569c96a15052704c`).replyOnce(200, {
  'type': 'committed'
})

// Mock `addFunds` transaction
mock.onPost(TRANSACTION_URL, {
  'protocol_version': 0,
  'service_id': 128,
  'message_id': 1,
  'signature': 'a1495b35248f7aefce93d9b7af431e2de6cc1ee523471a929b8a49045b0cf89f9a151627abfdc45671d866ce2a1b0e1282869ba233b1419acc87c5a5b064ef08',
  'body': {
    'amount': '50',
    'pub_key': 'ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600',
    'seed': '3730449745243792763'
  }
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}e4e1fa9f9dc0dad5763416d0f048b0d4179775de0249fc0927ceaf90b5beb16a`).replyOnce(200, {
  'type': 'in-pool'
})

mock.onGet(`${TRANSACTION_EXPLORER_URL}e4e1fa9f9dc0dad5763416d0f048b0d4179775de0249fc0927ceaf90b5beb16a`).replyOnce(200, {
  'type': 'committed'
})

mock.onGet(`${PROOF_URL}ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600`).replyOnce(200, addFundsTxNotAccepted)

mock.onGet(`${PROOF_URL}ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600`).replyOnce(200, addFundsTxAccepted)

// Mock `transfer` transaction
mock.onPost(TRANSACTION_URL, {
  'protocol_version': 0,
  'service_id': 128,
  'message_id': 0,
  'signature': '49cab16b383c9d8117fdf0bfed9bd1cc88f0638b5b50067084fbcdad64fcaf0b1240d7ff47c9009a6b3960b10bd882d362cae91a2753c2d68896a136601a8501',
  'body': {
    'amount': '100',
    'from': 'ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600',
    'to': 'bdf69b79d03f4debdcc0eb1ee36074094930badb17ee22888a7728ab42e5e493',
    'seed': '15549015379304915022'
  }
}).replyOnce(200)

mock.onGet(`${TRANSACTION_EXPLORER_URL}193d2932818a906b670833e0b0dcd5d16a045b73f7afeb9ea35d91856de204cf`).replyOnce(200, {
  'type': 'in-pool'
})

mock.onGet(`${TRANSACTION_EXPLORER_URL}193d2932818a906b670833e0b0dcd5d16a045b73f7afeb9ea35d91856de204cf`).replyOnce(200, {
  'type': 'committed'
})

mock.onGet(`${PROOF_URL}ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600`).replyOnce(200, transferTxNotAccepted)

mock.onGet(`${PROOF_URL}ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600`).replyOnce(200, transferTxAccepted)

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

  it('should get wallet proof and verify it', async () => {
    const keyPair = {
      publicKey: 'ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600',
      secretKey: '925e9a5787d97d720bf16adff5c6d3ebf81cf27b61a474e1cbc97f4f80dce4e0ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600'
    }
    const data = await Vue.prototype.$blockchain.getWallet(keyPair.publicKey)

    expect(data.block).toEqual({
      'height': '1966',
      'prev_hash': '5401485e3019dae35b445aa5d53c108e52cd6f60a1ea5c042461b13af65fcffa',
      'proposer_id': 3,
      'state_hash': 'c4bfe8907e01dba164ba7086a2f2c1dedaa63d32772e8cf88cb6cfa7e60676ca',
      'tx_count': 0,
      'tx_hash': '0000000000000000000000000000000000000000000000000000000000000000'
    })
    expect(data.wallet).toEqual({
      'balance': '100',
      'history_hash': '473fab385340f46b1e89f03124d05d849d7948ef11bc20ae569c96a15052704c',
      'history_len': '1',
      'name': 'John Doe',
      'pub_key': 'ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600'
    })
    expect(data.transactions).toEqual(expect.arrayContaining([
      {
        'body': {
          'name': 'John Doe',
          'pub_key': 'ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600'
        },
        'hash': '473fab385340f46b1e89f03124d05d849d7948ef11bc20ae569c96a15052704c',
        'message_id': 2,
        'protocol_version': 0,
        'service_id': 128,
        'signature': 'c28cc03f7b2bb41cac2c83896be31a293594100e8fdced2d439165f7e9227b271b464408c694df887548971db70b2cf4f287f0781f1a0e9dfe9bfa0f0b80b70a'
      }
    ]))
  })

  it('should create new wallet', async () => {
    const keyPair = {
      publicKey: 'ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600',
      secretKey: '925e9a5787d97d720bf16adff5c6d3ebf81cf27b61a474e1cbc97f4f80dce4e0ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600'
    }
    const name = 'John Doe'

    await expect(Vue.prototype.$blockchain.createWallet(keyPair, name)).resolves
  })

  it('should add funds', async () => {
    const keyPair = {
      publicKey: 'ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600',
      secretKey: '925e9a5787d97d720bf16adff5c6d3ebf81cf27b61a474e1cbc97f4f80dce4e0ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600'
    }
    const amountToAdd = '50'
    const seed = '3730449745243792763'

    await expect(Vue.prototype.$blockchain.addFunds(keyPair, amountToAdd, seed)).resolves
  })

  it('should transfer funds', async () => {
    const keyPair = {
      publicKey: 'ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600',
      secretKey: '925e9a5787d97d720bf16adff5c6d3ebf81cf27b61a474e1cbc97f4f80dce4e0ba78f4566a075958770ffd514cde99ed56bdb349fd95464a0b3ee1fb2459c600'
    }
    const receiver = 'bdf69b79d03f4debdcc0eb1ee36074094930badb17ee22888a7728ab42e5e493'
    const amountToTransfer = '100'
    const seed = '15549015379304915022'

    await expect(Vue.prototype.$blockchain.transfer(keyPair, receiver, amountToTransfer, seed)).resolves
  })
})
