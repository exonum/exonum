/**
 * @license
 * Copyright 2018 The Exonum Team
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

// Cryptocurrency service stub.

const exonum = require('exonum-client')
const fetch = require('node-fetch')
const expect = require('chai').expect

const SERVICE_ID = 1
const TX_CREATE_WALLET_ID = 0
const TX_TRANSFER_ID = 1
const SERVICE_URL = 'http://127.0.0.1:8000/api/services/cryptocurrency'

const TxCreateWallet = exports.TxCreateWallet = exonum.newMessage({
  size: 40,
  network_id: 0,
  protocol_version: 0,
  service_id: SERVICE_ID,
  message_id: TX_CREATE_WALLET_ID,

  fields: [
    { name: 'pub_key', type: exonum.PublicKey },
    { name: 'name', type: exonum.String }
  ]
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

const TxTranfer = exports.TxTranfer = exonum.newMessage({
  size: 80,
  network_id: 0,
  protocol_version: 0,
  service_id: SERVICE_ID,
  message_id: TX_TRANSFER_ID,

  fields: [
    { name: 'from', type: exonum.PublicKey },
    { name: 'to', type: exonum.PublicKey },
    { name: 'amount', type: exonum.Uint64 },
    { name: 'seed', type: exonum.Uint64 }
  ]
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

exports.service = {
  async createWallet (tx) {
    let response = await fetch(`${SERVICE_URL}/v1/wallets`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(tx)
    })
    response = await response.json()
    expect(response.tx_hash).to.equal(tx.hash())
  },

  async transfer (tx) {
    let response = await fetch(`${SERVICE_URL}/v1/wallets/transfer`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(tx)
    })
    response = await response.json()
    expect(response.tx_hash).to.equal(tx.hash())
  },

  async getWallet (pubkey) {
    const response = await fetch(`${SERVICE_URL}/v1/wallet/${pubkey}`)
    return response.json()
  }
}
