import Vue from 'vue/dist/vue'
import axios from 'axios'
import MockAdapter from 'axios-mock-adapter'
import * as Exonum from 'exonum-client'
import * as Blockchain from '../src/plugins/blockchain.js'

const mock = new MockAdapter(axios)
const hexRegex = /[0-9A-Fa-f]+/i;

Vue.use(Blockchain)

describe('Interaction with blockchain', () => {
  it('generate new signing key pair', () => {
    const keyPair = Vue.prototype.$blockchain.generateKeyPair()

    expect(keyPair.publicKey).toHaveLength(64)
    expect(keyPair.publicKey).toMatch(hexRegex)
    expect(keyPair.secretKey).toHaveLength(128)
    expect(keyPair.secretKey).toMatch(hexRegex)
  })
})
