import Vue from 'vue/dist/vue'
import axios from 'axios'
import MockAdapter from 'axios-mock-adapter'
import * as Exonum from 'exonum-client'
import * as Blockchain from '../src/plugins/blockchain.js'
import actual from './data/actual.json'
import walletProof from './data/3aaf1c6da235ac90aee412c505457fd4a43562f7fd5cb71aa883f2c729986d93.json'
import addFundsTxNotAccepted from './data/8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39-add-funds-not-accepted.json'
import addFundsTxAccepted from './data/8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39-add-funds-accepted.json'
import transferTxNotAccepted from './data/8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39-transfer-not-accepted.json'
import transferTxAccepted from './data/8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39-transfer-accepted.json'

const mock = new MockAdapter(axios)
const bigIntRegex = /[0-9]+/i;
const hexRegex = /[0-9A-Fa-f]+/i;

Vue.use(Blockchain)

mock.onGet('/api/services/configuration/v1/configs/actual').reply(200, actual)

mock.onGet('/api/services/cryptocurrency/v1/wallets/info/3aaf1c6da235ac90aee412c505457fd4a43562f7fd5cb71aa883f2c729986d93').replyOnce(200, walletProof)

mock.onPost('/api/services/cryptocurrency/v1/wallets/transaction', {
  'protocol_version': 0,
  'service_id': 128,
  'message_id': 2,
  'signature': '9736ea9a39db660c72fb8f866a675e0889a63b8d354db3a13b5666112e39e3fb1ed24bf5f3411f5628654695253f04d7675e463b7612564c59051f7183b1f30d',
  'body': {
    'pub_key': '814bca90d29c116b62e6d97a11a7178ac43920b6169654a79ed457a863b0f53e',
    'name': 'John Doe'
  }
}).replyOnce(200, {
  'tx_hash': '8055cd33cf11106f16321feb37777c3a92cbeaa23b9f7984a5b819ae51fee596'
})

mock.onPost('/api/services/cryptocurrency/v1/wallets/transaction', {
  'protocol_version': 0,
  'service_id': 128,
  'message_id': 1,
  'signature': '30e4140172a927f009868cfd9f8706409bf31335266a7172de68ee1addd7cf8212acd577536de8bcdf3cf4c26fc60425211c0337011c76cc0020c6b494abaf07',
  'body': {
    'amount': '50',
    'pub_key': '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39',
    'seed': '10731967336872248664'
  }
}).replyOnce(200, {
  'tx_hash': '1ef4ad31435588a8290a460d1bd0f57edce7ec2e34258693b25216818ed2b127'
})

mock.onGet('/api/explorer/v1/transactions/1ef4ad31435588a8290a460d1bd0f57edce7ec2e34258693b25216818ed2b127').replyOnce(200, {
  'type': 'in-pool'
})

mock.onGet('/api/explorer/v1/transactions/1ef4ad31435588a8290a460d1bd0f57edce7ec2e34258693b25216818ed2b127').replyOnce(200, {
  'type': 'committed'
})

mock.onGet('/api/explorer/v1/transactions/8055cd33cf11106f16321feb37777c3a92cbeaa23b9f7984a5b819ae51fee596').replyOnce(200, {
  'type': 'in-pool'
})

mock.onGet('/api/explorer/v1/transactions/8055cd33cf11106f16321feb37777c3a92cbeaa23b9f7984a5b819ae51fee596').replyOnce(200, {
  'type': 'committed'
})

mock.onGet('/api/explorer/v1/transactions/0728ebfd50515a572deed796b7e2ab55f879fe999f8f754ff36a4a25e1efcbcc').replyOnce(200, {
  'type': 'in-pool'
})

mock.onGet('/api/explorer/v1/transactions/0728ebfd50515a572deed796b7e2ab55f879fe999f8f754ff36a4a25e1efcbcc').replyOnce(200, {
  'type': 'committed'
})

mock.onGet('/api/services/cryptocurrency/v1/wallets/info/8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39').replyOnce(200, addFundsTxNotAccepted)

mock.onGet('/api/services/cryptocurrency/v1/wallets/info/8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39').replyOnce(200, addFundsTxAccepted)

mock.onPost('/api/services/cryptocurrency/v1/wallets/transaction', {
  'protocol_version': 0,
  'service_id': 128,
  'message_id': 0,
  'signature': 'f857d05e4daad30ec73e2b4cf2822b23b1125d0468f8498c58fa47641537e90dae878ed0a5ba88cb2f33e68945e86c7a04afa2094601f9639a90da77f968b00c',
  'body': {
    'amount': '25',
    'from': '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39',
    'to': '814bca90d29c116b62e6d97a11a7178ac43920b6169654a79ed457a863b0f53e',
    'seed': '11266655484997490378'
  }
}).replyOnce(200, {
  'tx_hash': '0728ebfd50515a572deed796b7e2ab55f879fe999f8f754ff36a4a25e1efcbcc'
})

mock.onGet('/api/services/cryptocurrency/v1/wallets/info/8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39').replyOnce(200, transferTxNotAccepted)

mock.onGet('/api/services/cryptocurrency/v1/wallets/info/8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39').replyOnce(200, transferTxAccepted)

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
      publicKey: '3aaf1c6da235ac90aee412c505457fd4a43562f7fd5cb71aa883f2c729986d93',
      secretKey: 'd8545b0a31af84198a3c72fa777b8167e2226746c368875dc98b7f2aacb2429e3aaf1c6da235ac90aee412c505457fd4a43562f7fd5cb71aa883f2c729986d93'
    }
    const data = await Vue.prototype.$blockchain.getWallet(keyPair.publicKey)

    expect(data.block).toEqual({
      'height': '48417',
      'prev_hash': '35f0f88be280a45f4df84d195117a1e238e8ace60a4e3a6cbbf76a41d6e35372',
      'proposer_id': 2,
      'schema_version': 0,
      'state_hash': 'f3065e4a3d260e3a6002357a214e184ded8f1c40428728f63eefff213302f4f5',
      'tx_count': 0,
      'tx_hash': '0000000000000000000000000000000000000000000000000000000000000000'
    })
    expect(data.wallet).toEqual({
      'balance': '100',
      'history_hash': 'deb6cc887d84a0973fe60cd23958d94af64fda8e768283c52d44e2864d3c4585',
      'history_len': '1',
      'name': 'Rembo',
      'pub_key': '3aaf1c6da235ac90aee412c505457fd4a43562f7fd5cb71aa883f2c729986d93'
    })
    expect(data.transactions).toEqual(expect.arrayContaining([{
      'body': {
        'name': 'Rembo',
        'pub_key': '3aaf1c6da235ac90aee412c505457fd4a43562f7fd5cb71aa883f2c729986d93'
      },
      'hash': 'deb6cc887d84a0973fe60cd23958d94af64fda8e768283c52d44e2864d3c4585',
      'message_id': 2,
      'protocol_version': 0,
      'service_id': 128,
      'signature': '0235995ce5ae5ddc40b7b924b33a768d671d6d702724ba4b144ecaf56c124ea2912dd34a048af3ffcb60193e0ebeeb2998f6b563fec31cf19ba584d4a6ecc30e'
    }]))
  })

  it('should create new wallet', async () => {
    const keyPair = {
      publicKey: '814bca90d29c116b62e6d97a11a7178ac43920b6169654a79ed457a863b0f53e',
      secretKey: '0a8779e7a5edeaf71455a06205493fd5c4b4623d24755edc4013238145cd7982814bca90d29c116b62e6d97a11a7178ac43920b6169654a79ed457a863b0f53e'
    }
    const name = 'John Doe'
    const data = await Vue.prototype.$blockchain.createWallet(keyPair, name)

    expect(data.data).toEqual({
      'tx_hash': '8055cd33cf11106f16321feb37777c3a92cbeaa23b9f7984a5b819ae51fee596'
    })
  })

  it('should add funds', async () => {
    const keyPair = {
      publicKey: '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39',
      secretKey: 'f34303dbf7637f2e549be572e7e9b86c6ad05c9253c028b310d4a670664c08da8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39'
    }
    const amountToAdd = '50'
    const seed = '10731967336872248664'
    const data = await Vue.prototype.$blockchain.addFunds(keyPair, amountToAdd, seed)

    expect(data.block).toEqual({
      'height': '52684',
      'prev_hash': 'e869d2531cf5e74ee30049ff602f250eb81a59059eec99362fa0dbd5ec840876',
      'proposer_id': 1,
      'schema_version': 0,
      'state_hash': '1960e6c1df24282a6bfbaccdd889f05e5f6f373d760a63c0a5403401f3569a54',
      'tx_count': 1,
      'tx_hash': '1ef4ad31435588a8290a460d1bd0f57edce7ec2e34258693b25216818ed2b127'
    })
    expect(data.wallet).toEqual({
      'balance': '150',
      'history_hash': '0f5bf0a9e4935790f244e4534508b3dd58ecb31fc85ba8918ac7d0e032aed456',
      'history_len': '2',
      'name': 'Alexa',
      'pub_key': '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39'
    })
    expect(data.transactions).toEqual(expect.arrayContaining([{
        'body': {
          'name': 'Alexa',
          'pub_key': '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39'
        },
        'hash': '9efbdb64fcf4372593db83e9564e1cfd896a70ef6910e75cef43db1d15b15500',
        'message_id': 2,
        'protocol_version': 0,
        'service_id': 128,
        'signature': 'db69089d985afd661d0f0249dc4c2aeb9214cacb4bbcfa1a6a9751fcc6605cf0c4c59196386d963e9175e27fe0007a6e9a3ded438d2bc3ed1d6681b092f30109'
      },
      {
        'body': {
          'amount': '50',
          'pub_key': '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39',
          'seed': '10731967336872248664'
        },
        'hash': '1ef4ad31435588a8290a460d1bd0f57edce7ec2e34258693b25216818ed2b127',
        'message_id': 1,
        'protocol_version': 0,
        'service_id': 128,
        'signature': '30e4140172a927f009868cfd9f8706409bf31335266a7172de68ee1addd7cf8212acd577536de8bcdf3cf4c26fc60425211c0337011c76cc0020c6b494abaf07'
      }
    ]))
  })

  it('should transfer funds', async () => {
    const keyPair = {
      publicKey: '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39',
      secretKey: 'f34303dbf7637f2e549be572e7e9b86c6ad05c9253c028b310d4a670664c08da8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39'
    }
    const receiver = '814bca90d29c116b62e6d97a11a7178ac43920b6169654a79ed457a863b0f53e'
    const amountToTransfer = '25'
    const seed = '11266655484997490378'
    const data = await Vue.prototype.$blockchain.transfer(keyPair, receiver, amountToTransfer, seed)

    expect(data.block).toEqual({
      'height': '56421',
      'prev_hash': '686d0433f23e3aceebb7c81a02410020909066e84f2afe408a793491abdfa4bf',
      'proposer_id': 2,
      'schema_version': 0,
      'state_hash': '9b24670821598b46e0f40e62392153599f052c2d1d4df05bf3d18487d1d26b8b',
      'tx_count': 1,
      'tx_hash': '0728ebfd50515a572deed796b7e2ab55f879fe999f8f754ff36a4a25e1efcbcc'
    })
    expect(data.wallet).toEqual({
      'balance': '125',
      'history_hash': 'f35b4b3576d1b3a2f30d4ac81f5ff2e1faa816948007e30b60a2c1e088ce239b',
      'history_len': '3',
      'name': 'Alexa',
      'pub_key': '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39'
    })
    expect(data.transactions).toEqual(expect.arrayContaining([{
        'body': {
          'name': 'Alexa',
          'pub_key': '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39'
        },
        'hash': '9efbdb64fcf4372593db83e9564e1cfd896a70ef6910e75cef43db1d15b15500',
        'message_id': 2,
        'protocol_version': 0,
        'service_id': 128,
        'signature': 'db69089d985afd661d0f0249dc4c2aeb9214cacb4bbcfa1a6a9751fcc6605cf0c4c59196386d963e9175e27fe0007a6e9a3ded438d2bc3ed1d6681b092f30109'
      },
      {
        'body': {
          'amount': '50',
          'pub_key': '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39',
          'seed': '10731967336872248664'
        },
        'hash': '1ef4ad31435588a8290a460d1bd0f57edce7ec2e34258693b25216818ed2b127',
        'message_id': 1,
        'protocol_version': 0,
        'service_id': 128,
        'signature': '30e4140172a927f009868cfd9f8706409bf31335266a7172de68ee1addd7cf8212acd577536de8bcdf3cf4c26fc60425211c0337011c76cc0020c6b494abaf07'
      },
      {
        'body': {
          'amount': '25',
          'from': '8a2f2eb5302deeb8376fd347f2704a066cddfc92b3073f3668511b4ebc8fda39',
          'seed': '11266655484997490378',
          'to': '814bca90d29c116b62e6d97a11a7178ac43920b6169654a79ed457a863b0f53e'
        },
        'hash': '0728ebfd50515a572deed796b7e2ab55f879fe999f8f754ff36a4a25e1efcbcc',
        'message_id': 0,
        'protocol_version': 0,
        'service_id': 128,
        'signature': 'f857d05e4daad30ec73e2b4cf2822b23b1125d0468f8498c58fa47641537e90dae878ed0a5ba88cb2f33e68945e86c7a04afa2094601f9639a90da77f968b00c'
      }
    ]))
  })
})
