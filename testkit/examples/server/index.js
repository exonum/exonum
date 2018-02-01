/**
 * @license
 * Copyright 2017 The Exonum Team
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *    http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
/* eslint-env node,mocha */

const exonum = require('exonum-client')
const fetch = require('node-fetch')
const expect = require('chai').expect

// // // // Testkit functions // // // //

const TESTKIT_URL = 'http://127.0.0.1:9000/api/testkit'

async function createBlock (txHashes) {
  const body = (txHashes === undefined) ? { } : {
    tx_hashes: txHashes
  }

  const response = await fetch(TESTKIT_URL + '/v1/blocks', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body)
  })
  return response.json()
}

async function rollbackToHeight (height) {
  const response = await fetch(TESTKIT_URL + '/v1/blocks/' + (height + 1), {
    method: 'DELETE'
  })
  return response.json()
}

async function getBlockchainHeight () {
  let response = await fetch(TESTKIT_URL + '/v1/status')
  response = await response.json()
  return response.height
}

describe('CurrencyService', function () {
  this.slow(500)

  // // // // Service constants // // // //

  const SERVICE_ID = 1
  const TX_CREATE_WALLET_ID = 1
  const TX_TRANSFER_ID = 2
  const SERVICE_URL = 'http://127.0.0.1:8000/api/services/cryptocurrency'

  const TxCreateWallet = exonum.newMessage({
    size: 40,
    network_id: 0,
    protocol_version: 0,
    service_id: SERVICE_ID,
    message_id: TX_CREATE_WALLET_ID,

    fields: {
      pub_key: { type: exonum.PublicKey, size: 32, from: 0, to: 32 },
      name: { type: exonum.String, size: 8, from: 32, to: 40 }
    }
  })

  TxCreateWallet.new = function (body) {
    return {
      network_id: 0,
      protocol_version: 0,
      service_id: SERVICE_ID,
      message_id: TX_CREATE_WALLET_ID,
      body,

      hash () {
        TxCreateWallet.signature = this.signature
        const hash = TxCreateWallet.hash(this.body)
        delete TxCreateWallet.signature
        return hash
      }
    }
  }

  const TxTranfer = exonum.newMessage({
    size: 80,
    network_id: 0,
    protocol_version: 0,
    service_id: SERVICE_ID,
    message_id: TX_TRANSFER_ID,

    fields: {
      from: { type: exonum.PublicKey, size: 32, from: 0, to: 32 },
      to: { type: exonum.PublicKey, size: 32, from: 32, to: 64 },
      amount: { type: exonum.Uint64, size: 8, from: 64, to: 72 },
      seed: { type: exonum.Uint64, size: 8, from: 72, to: 80 }
    }
  })

  TxTranfer.new = function (body) {
    return {
      network_id: 0,
      protocol_version: 0,
      service_id: SERVICE_ID,
      message_id: TX_TRANSFER_ID,
      body,

      hash () {
        TxTranfer.signature = this.signature
        const hash = TxTranfer.hash(this.body)
        delete TxTranfer.signature
        return hash
      }
    }
  }

  // // // // Service-specific functions // // // //

  async function createWallet (tx) {
    let response = await fetch(SERVICE_URL + '/v1/wallets', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(tx)
    })
    response = await response.json()
    expect(response.tx_hash).to.equal(tx.hash())
  }

  async function transfer (tx) {
    let response = await fetch(SERVICE_URL + '/v1/wallets/transfer', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(tx)
    })
    response = await response.json()
    expect(response.tx_hash).to.equal(tx.hash())
  }

  async function getWallet (pubkey) {
    const response = await fetch(SERVICE_URL + '/v1/wallet/' + pubkey)
    return response.json()
  }

  // // // // Tests // // // //

  beforeEach(async () => {
    await rollbackToHeight(0)
  })

  it('should create wallet', async () => {
    const { publicKey, secretKey } = exonum.keyPair()
    const tx = TxCreateWallet.new({
      pub_key: publicKey,
      name: 'Alice'
    })
    tx.signature = TxCreateWallet.sign(secretKey, tx.body)

    await createWallet(tx)
    await createBlock()
    expect(await getBlockchainHeight()).to.equal(1)
    const wallet = await getWallet(publicKey)
    expect(wallet.name).to.equal('Alice')
    expect('' + wallet.balance).to.equal('100')
  })

  it('should perform transfer between wallets', async () => {
    const { publicKey: alicePK, secretKey: aliceKey } = exonum.keyPair()
    const txAlice = TxCreateWallet.new({
      pub_key: alicePK,
      name: 'Alice'
    })
    txAlice.signature = TxCreateWallet.sign(aliceKey, txAlice.body)

    const { publicKey: bobPK, secretKey: bobKey } = exonum.keyPair()
    const txBob = TxCreateWallet.new({
      pub_key: bobPK,
      name: 'Bob'
    })
    txBob.signature = TxCreateWallet.sign(bobKey, txBob.body)

    const transferTx = TxTranfer.new({
      from: alicePK,
      to: bobPK,
      amount: '15',
      seed: '0'
    })
    transferTx.signature = TxTranfer.sign(aliceKey, transferTx.body)

    await Promise.all([
      createWallet(txAlice),
      createWallet(txBob),
      transfer(transferTx)
    ])
    await createBlock([
      txAlice.hash(),
      txBob.hash(),
      transferTx.hash()
    ])
    expect(await getBlockchainHeight()).to.equal(1)
    const [aliceWallet, bobWallet] = await Promise.all([
      getWallet(alicePK),
      getWallet(bobPK)
    ])
    expect('' + aliceWallet.balance).to.equal('85')
    expect('' + bobWallet.balance).to.equal('115')
  })

  it('should not perform transfer between wallets if the receiver is unknown', async () => {
    const { publicKey: alicePK, secretKey: aliceKey } = exonum.keyPair()
    const txAlice = TxCreateWallet.new({
      pub_key: alicePK,
      name: 'Alice'
    })
    txAlice.signature = TxCreateWallet.sign(aliceKey, txAlice.body)

    const { publicKey: bobPK, secretKey: bobKey } = exonum.keyPair()
    const txBob = TxCreateWallet.new({
      pub_key: bobPK,
      name: 'Bob'
    })
    txBob.signature = TxCreateWallet.sign(bobKey, txBob.body)

    const transferTx = TxTranfer.new({
      from: alicePK,
      to: bobPK,
      amount: '15',
      seed: '0'
    })
    transferTx.signature = TxTranfer.sign(aliceKey, transferTx.body)

    await Promise.all([
      createWallet(txAlice),
      createWallet(txBob),
      transfer(transferTx)
    ])
    await createBlock([
      txAlice.hash(),
      transferTx.hash()
    ])
    const [aliceWallet, bobWallet] = await Promise.all([
      getWallet(alicePK),
      getWallet(bobPK)
    ])
    expect('' + aliceWallet.balance).to.equal('100')
    expect(bobWallet).to.equal('Wallet not found')
  })
})
