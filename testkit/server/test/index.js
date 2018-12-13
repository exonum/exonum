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
/* eslint-env node,mocha */


const exonum = require('exonum-client')
const expect = require('chai').expect
const testkit = require('./testkit')
const {
  service,
  TxTranfer
} = require('./service')

describe('CurrencyService', function () {
  this.slow(500)

  beforeEach(async () => {
    await testkit.rollbackToHeight(0)
  })

  it('should create wallet', async () => {
    const {
      publicKey,
      secretKey
    } = exonum.keyPair()
    const tx = service.createWalletTransaction(publicKey)
    await service.transactionSend(secretKey, tx, {
      name: 'Kate'
    })

    await testkit.createBlock()
    expect(await testkit.getBlockchainHeight()).to.equal(1)

    const wallet = await service.getWallet(publicKey)
    expect('' + wallet.balance).to.equal('100')
    expect(wallet.name).to.equal('Kate')
  })


  it('should perform transfer between wallets', async () => {
    const {
      publicKey: alicePK,
      secretKey: aliceKey
    } = exonum.keyPair()
    const txAlice = service.createWalletTransaction(alicePK)

    const {
      publicKey: bobPK,
      secretKey: bobKey
    } = exonum.keyPair()
    const txBob = service.createWalletTransaction(bobPK)

    const transferTx = service.createTransferTransaction(alicePK)

    await Promise.all([
      service.transactionSend(aliceKey, txAlice, {
        name: 'Alice'
      }),
      service.transactionSend(bobKey, txBob, {
        name: 'Bob'
      }),
      service.transactionSend(aliceKey, transferTx, {
        to: {
          data: Uint8Array.from(exonum.hexadecimalToUint8Array(bobPK))
        },
        amount: '15',
        seed: '0'
      })
    ])
    await testkit.createBlock([
      txAlice.hash,
      txBob.hash,
      transferTx.hash
    ])
    expect(await testkit.getBlockchainHeight()).to.equal(1)
    const [aliceWallet, bobWallet] = await Promise.all([
      service.getWallet(alicePK),
      service.getWallet(bobPK)
    ])
    expect('' + aliceWallet.balance).to.equal('85')
    expect('' + bobWallet.balance).to.equal('115')
  })

  it('should not perform transfer between wallets if the receiver is unknown', async () => {
    const {
      publicKey: alicePK,
      secretKey: aliceKey
    } = exonum.keyPair()
    const txAlice = service.createWalletTransaction(alicePK)

    const {
      publicKey: bobPK,
      secretKey: bobKey
    } = exonum.keyPair()
    const txBob = service.createWalletTransaction(bobPK)

    const transferTx = service.createTransferTransaction(alicePK)

    await Promise.all([
      service.transactionSend(aliceKey, txAlice, {
        name: 'Alice'
      }),
      service.transactionSend(bobKey, txBob, {
        name: 'Bob'
      }),
      service.transactionSend(aliceKey, transferTx, {
        to: {
          data: Uint8Array.from(exonum.hexadecimalToUint8Array(bobPK))
        },
        amount: '15',
        seed: '0'
      })
    ])
    await testkit.createBlock([
      txAlice.hash,
      transferTx.hash
    ])
    const [aliceWallet, bobWallet] = await Promise.all([
      service.getWallet(alicePK),
      service.getWallet(bobPK)
    ])
    expect('' + aliceWallet.balance).to.equal('100')
    expect(bobWallet).to.equal('Wallet not found')
  })
})
