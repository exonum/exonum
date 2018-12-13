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

const proto = require('./stubs.js')
const $protobuf = require('protobufjs/light')
const Root = $protobuf.Root
const Type = $protobuf.Type
const Field = $protobuf.Field

let root = new Root()

const exonum = require('exonum-client')
const fetch = require('node-fetch')
const expect = require('chai').expect

const SERVICE_ID = 1
const TX_CREATE_WALLET_ID = 0
const TX_TRANSFER_ID = 1
const SERVICE_URL = 'http://127.0.0.1:8000/api/explorer'
const EXPLORER_URL = 'http://127.0.0.1:8000/api/services/cryptocurrency'

function haveTxBody (type, data, secretKey) {

  // clone type
  const typeCopy = exonum.newTransaction(type)

  // sign transaction
  typeCopy.signature = typeCopy.sign(secretKey, data)

  // serialize transaction header and body
  const buffer = typeCopy.serialize(data)

  // convert buffer into hexadecimal string
  const txBody = exonum.uint8ArrayToHexadecimal(new Uint8Array(buffer))

  // get transaction hash
  const txHash = exonum.hash(buffer)

  return {
    tx_body: txBody,
    tx_hash: txHash
  }
}

exports.service = {
  createWalletTransaction (author) {
    const tx = exonum.newTransaction({
      service_id: SERVICE_ID,
      message_id: TX_CREATE_WALLET_ID,
      author,
      schema: proto.exonum.examples.cryptocurrency.TxCreateWallet 
    }) 

    return tx
  },

  createTransferTransaction (author) {
    const tx = exonum.newTransaction({
      service_id: SERVICE_ID,
      message_id: TX_TRANSFER_ID,
      author,
      schema: proto.exonum.examples.cryptocurrency.TxTransfer
    }) 

    return tx
  },

  async transactionSend (sk, tx, body) {

    const { tx_body, tx_hash } = haveTxBody(tx, body, sk)

    let response = await fetch(`${SERVICE_URL}/v1/transactions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ tx_body: tx_body })
    })
    response = await response.json()
    expect(response.tx_hash).to.equal(tx_hash)
    tx.hash = response.tx_hash;
  },

  async getWallet (pubkey) {
    const response = await fetch(`${EXPLORER_URL}/v1/wallet?pub_key=${pubkey}`)
    return response.json()
  }
}
