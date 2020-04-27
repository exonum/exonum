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

// Testkit stub.

const fetch = require('node-fetch')

const TESTKIT_URL = 'http://127.0.0.1:9000/api/testkit'

module.exports = {
  async createBlock (txHashes) {
    const body = (txHashes === undefined) ? { } : {
      tx_hashes: txHashes
    }

    const response = await fetch(`${TESTKIT_URL}/v1/blocks/create`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body)
    })
    return response.json()
  },

  async rollbackToHeight (height) {
    const response = await fetch(`${TESTKIT_URL}/v1/blocks/rollback`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(height + 1)
    })
    return response.json()
  },

  async getBlockchainHeight () {
    let response = await fetch(`${TESTKIT_URL}/v1/status`)
    response = await response.json()
    return response.height
  }
}
