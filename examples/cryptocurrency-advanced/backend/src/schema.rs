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

use exonum_merkledb::{IndexAccess, ObjectHash, ProofListIndex, RawProofMapIndex};

use exonum::crypto::{Hash, PublicKey};

use crate::{wallet::Wallet, INITIAL_BALANCE};

/// Database schema for the cryptocurrency.
#[derive(Debug)]
pub struct Schema<'a, T> {
    service_name: &'a str,
    access: T,
}

impl<'a, T> Schema<'a, T>
where
    T: IndexAccess,
{
    /// Creates a new schema from the database view.
    pub fn new(service_name: &'a str, access: T) -> Self {
        Schema {
            access,
            service_name,
        }
    }

    /// Returns `ProofMapIndex` with wallets.
    pub fn wallets(&self) -> RawProofMapIndex<T, PublicKey, Wallet> {
        RawProofMapIndex::new(
            [self.service_name, ".wallets"].concat(),
            self.access.clone(),
        )
    }

    /// Returns history of the wallet with the given public key.
    pub fn wallet_history(&self, public_key: &PublicKey) -> ProofListIndex<T, Hash> {
        ProofListIndex::new_in_family(
            [self.service_name, ".wallet_history"].concat(),
            public_key,
            self.access.clone(),
        )
    }

    /// Returns wallet for the given public key.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }

    /// Returns the state hash of cryptocurrency service.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.wallets().object_hash()]
    }

    /// Increase balance of the wallet and append new record to its history.
    ///
    /// Panics if there is no wallet with given public key.
    pub fn increase_wallet_balance(&mut self, wallet: Wallet, amount: u64, transaction: Hash) {
        let wallet = {
            let mut history = self.wallet_history(&wallet.pub_key);
            history.push(transaction);
            let history_hash = history.object_hash();
            let balance = wallet.balance;
            wallet.set_balance(balance + amount, &history_hash)
        };
        self.wallets().put(&wallet.pub_key, wallet.clone());
    }

    /// Decrease balance of the wallet and append new record to its history.
    ///
    /// Panics if there is no wallet with given public key.
    pub fn decrease_wallet_balance(&mut self, wallet: Wallet, amount: u64, transaction: Hash) {
        let wallet = {
            let mut history = self.wallet_history(&wallet.pub_key);
            history.push(transaction);
            let history_hash = history.object_hash();
            let balance = wallet.balance;
            wallet.set_balance(balance - amount, &history_hash)
        };
        self.wallets().put(&wallet.pub_key, wallet.clone());
    }

    /// Create new wallet and append first record to its history.
    pub fn create_wallet(&mut self, key: &PublicKey, name: &str, transaction: Hash) {
        let wallet = {
            let mut history = self.wallet_history(key);
            history.push(transaction);
            let history_hash = history.object_hash();
            Wallet::new(key, name, INITIAL_BALANCE, history.len(), &history_hash)
        };
        self.wallets().put(key, wallet);
    }
}
