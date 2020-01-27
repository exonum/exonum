/**
 * @license
 * Copyright 2020 The Exonum Team
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

const { expect } = require('chai')
const exonum = require('exonum-client')
const fetch = require('node-fetch')
const proto = require('./stubs.js')

const SERVICE_ID = 101
const EXPLORER_URL = 'http://127.0.0.1:8000/api/explorer/v1/transactions'
const SERVICE_URL = 'http://127.0.0.1:8000/api/services/cryptocurrency'

const CreateWallet = new exonum.Transaction({
  schema: proto.exonum.examples.cryptocurrency.TxCreateWallet,
  serviceId: SERVICE_ID,
  methodId: 0
})
const Transfer = new exonum.Transaction({
  schema: proto.exonum.examples.cryptocurrency.TxTransfer,
  serviceId: SERVICE_ID,
  methodId: 1
})

exports.service = {
  createWallet (keyPair, name) {
    return CreateWallet.create({ name }, keyPair)
  },

  createTransfer (keyPair, to, amount) {
    const payload = {
      to: { data: exonum.hexadecimalToUint8Array(to) },
      amount,
      seed: exonum.randomUint64()
    }
    return Transfer.create(payload, keyPair)
  },

  async sendTransaction (transaction) {
    // It is impossible to use `exonum.send` here because it waits until transaction is committed,
    // which does not happen automatically in the testkit.
    const bytes = exonum.uint8ArrayToHexadecimal(transaction.serialize())
    const response = await fetch(EXPLORER_URL, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tx_body: bytes })
    })
    const { tx_hash: hash } = await response.json()
    expect(hash).to.equal(transaction.hash())
    return hash
  },

  async getWallet (pubkey) {
    const response = await fetch(`${SERVICE_URL}/v1/wallet?pub_key=${pubkey}`)
    return response.json()
  }
}
