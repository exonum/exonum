// Copyright 2018 The Exonum Team
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

use exonum::blockchain::gen_prefix;
use exonum::crypto::{Hash, PublicKey};
use exonum::storage::{Fork, ProofListIndex, ProofMapIndex, Snapshot};

use std::fmt;

use INITIAL_BALANCE;
use wallet::Wallet;

/// Database schema for the cryptocurrency.
pub struct CurrencySchema<T> {
    view: T,
}

impl<T> AsMut<T> for CurrencySchema<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.view
    }
}

impl<T> fmt::Debug for CurrencySchema<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CurrencySchema {{}}")
    }
}

impl<T> CurrencySchema<T>
where
    T: AsRef<Snapshot>,
{
    /// Constructs schema from the database view.
    pub fn new(view: T) -> Self {
        CurrencySchema { view }
    }

    /// Returns `MerklePatriciaTable` with wallets.
    pub fn wallets(&self) -> ProofMapIndex<&T, PublicKey, Wallet> {
        ProofMapIndex::new("cryptocurrency.wallets", &self.view)
    }

    /// Returns history of the wallet with the given public key.
    pub fn wallet_history(&self, public_key: &PublicKey) -> ProofListIndex<&T, Hash> {
        ProofListIndex::with_prefix(
            "cryptocurrency.wallet_history",
            gen_prefix(public_key),
            &self.view,
        )
    }

    /// Returns wallet for the given public key.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }

    /// Returns database state hash.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.wallets().root_hash()]
    }
}

/// Implementation of mutable methods.
impl<'a> CurrencySchema<&'a mut Fork> {
    /// Returns mutable `MerklePatriciaTable` with wallets.
    pub fn wallets_mut(&mut self) -> ProofMapIndex<&mut Fork, PublicKey, Wallet> {
        ProofMapIndex::new("cryptocurrency.wallets", &mut self.view)
    }

    /// Returns history for the wallet by the given public key.
    pub fn wallet_history_mut(
        &mut self,
        public_key: &PublicKey,
    ) -> ProofListIndex<&mut Fork, Hash> {
        ProofListIndex::with_prefix(
            "cryptocurrency.wallet_history",
            gen_prefix(public_key),
            &mut self.view,
        )
    }

    /// Update balance of the wallet and append new record to its history.
    ///
    /// Panics if there is no wallet with given public key.
    pub fn set_wallet_balance(&mut self, key: &PublicKey, balance: u64, transaction: &Hash) {
        let wallet = {
            let wallet = self.wallet(key).unwrap();
            let mut history = self.wallet_history_mut(key);
            history.push(*transaction);
            let history_hash = history.root_hash();
            wallet.set_balance(balance, &history_hash)
        };
        self.wallets_mut().put(key, wallet);
    }

    /// Create new wallet and append first record to its history.
    pub fn create_wallet(&mut self, key: &PublicKey, name: &str, transaction: &Hash) {
        let wallet = {
            let mut history = self.wallet_history_mut(key);
            history.push(*transaction);
            let history_hash = history.root_hash();
            Wallet::new(key, name, INITIAL_BALANCE, history.len(), &history_hash)
        };
        self.wallets_mut().put(key, wallet);
    }
}
