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
/* eslint-env node,mocha */

require('regenerator-runtime/runtime')

const exonum = require('exonum-client')
const expect = require('chai').expect
const testkit = require('./testkit')
const { service } = require('./service')

describe('CurrencyService', function () {
  this.slow(500)

  beforeEach(async () => {
    await testkit.rollbackToHeight(0)
  })

  it('should create wallet', async () => {
    const alice = exonum.keyPair()
    const tx = service.createWallet(alice, 'Alice')
    await service.sendTransaction(tx)

    await testkit.createBlock()
    expect(await testkit.getBlockchainHeight()).to.equal(1)

    const wallet = await service.getWallet(alice.publicKey)
    expect('' + wallet.balance).to.equal('100')
    expect(wallet.name).to.equal('Alice')
  })

  it('should perform transfer between wallets', async () => {
    const alice = exonum.keyPair()
    const txAlice = service.createWallet(alice, 'Alice')

    const bob = exonum.keyPair()
    const txBob = service.createWallet(bob, 'Bob')

    const transferTx = service.createTransfer(alice, bob.publicKey, 15)

    await Promise.all([
      service.sendTransaction(txAlice),
      service.sendTransaction(txBob),
      service.sendTransaction(transferTx)
    ])
    await testkit.createBlock([
      txAlice.hash(),
      txBob.hash(),
      transferTx.hash()
    ])
    expect(await testkit.getBlockchainHeight()).to.equal(1)
    const [aliceWallet, bobWallet] = await Promise.all([
      service.getWallet(alice.publicKey),
      service.getWallet(bob.publicKey)
    ])
    expect('' + aliceWallet.balance).to.equal('85')
    expect('' + bobWallet.balance).to.equal('115')
  })

  it('should not perform transfer between wallets if the receiver is unknown', async () => {
    const alice = exonum.keyPair()
    const txAlice = service.createWallet(alice, 'Alice')
    const bob = exonum.keyPair()
    const txBob = service.createWallet(bob, 'Bob')

    const transferTx = service.createTransfer(alice, bob.publicKey, 15)

    await Promise.all([
      service.sendTransaction(txAlice),
      service.sendTransaction(txBob),
      service.sendTransaction(transferTx)
    ])

    // Note that the Bob's wallet creation transaction is missing
    await testkit.createBlock([
      txAlice.hash(),
      transferTx.hash()
    ])
    const [aliceWallet, bobError] = await Promise.all([
      service.getWallet(alice.publicKey),
      service.getWallet(bob.publicKey)
    ])
    expect('' + aliceWallet.balance).to.equal('100')
    expect(bobError).to.deep.equal({
      title: 'Wallet not found',
      source: '101:cryptocurrency'
    })
  })
})
