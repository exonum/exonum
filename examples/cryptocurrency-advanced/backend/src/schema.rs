// Copyright 2019 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Cryptocurrency database schema.

use exonum_merkledb::{AccessExt, IndexAccessMut, ObjectHash, ProofListIndex, ProofMapIndex};

use exonum::crypto::{Hash, PublicKey};

use crate::{wallet::Wallet, INITIAL_BALANCE};

const NOT_INITIALIZED: &str = "Cryptocurrency schema is not initialized";

/// Database schema for the cryptocurrency.
#[derive(Debug)]
pub struct Schema<T: AccessExt> {
    /// Map of wallet keys to information about the corresponding account.
    pub wallets: ProofMapIndex<T::Base, PublicKey, Wallet>,
    access: T,
}

impl<T: AccessExt> Schema<T> {
    /// Creates a new schema from the database view.
    pub fn new(access: T) -> Self {
        Self {
            wallets: access.proof_map("wallets").expect(NOT_INITIALIZED),
            access,
        }
    }

    /// Returns history of the wallet with the given public key.
    pub fn wallet_history(&self, public_key: &PublicKey) -> Option<ProofListIndex<T::Base, Hash>> {
        self.access.proof_list(("wallet_history", public_key))
    }

    /// Returns wallet for the given public key.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets.get(pub_key)
    }

    /// Returns the state hash of cryptocurrency service.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.wallets.object_hash()]
    }
}

impl<T> Schema<T>
where
    T: AccessExt,
    T::Base: IndexAccessMut,
{
    pub(crate) fn initialize(access: T) -> Self {
        Self {
            wallets: access.ensure_proof_map("wallets"),
            access,
        }
    }

    fn ensure_wallet_history(&self, public_key: &PublicKey) -> ProofListIndex<T::Base, Hash> {
        self.access
            .ensure_proof_list(("wallet_history", public_key))
    }

    /// Increase balance of the wallet and append new record to its history.
    ///
    /// Panics if there is no wallet with given public key.
    pub(crate) fn increase_wallet_balance(
        &mut self,
        wallet: Wallet,
        amount: u64,
        transaction: Hash,
    ) {
        let mut history = self.ensure_wallet_history(&wallet.pub_key);
        history.push(transaction);
        let history_hash = history.object_hash();
        let balance = wallet.balance;
        let wallet = wallet.set_balance(balance + amount, &history_hash);
        let wallet_key = wallet.pub_key;
        self.wallets.put(&wallet_key, wallet);
    }

    /// Decrease balance of the wallet and append new record to its history.
    ///
    /// Panics if there is no wallet with given public key.
    pub(crate) fn decrease_wallet_balance(
        &mut self,
        wallet: Wallet,
        amount: u64,
        transaction: Hash,
    ) {
        let mut history = self.ensure_wallet_history(&wallet.pub_key);
        history.push(transaction);
        let history_hash = history.object_hash();
        let balance = wallet.balance;
        let wallet = wallet.set_balance(balance - amount, &history_hash);
        let wallet_key = wallet.pub_key;
        self.wallets.put(&wallet_key, wallet);
    }

    /// Create new wallet and append first record to its history.
    pub(crate) fn create_wallet(&mut self, key: &PublicKey, name: &str, transaction: Hash) {
        let mut history = self.ensure_wallet_history(key);
        history.push(transaction);
        let history_hash = history.object_hash();
        let wallet = Wallet::new(key, name, INITIAL_BALANCE, history.len(), &history_hash);
        self.wallets.put(key, wallet);
    }
}
