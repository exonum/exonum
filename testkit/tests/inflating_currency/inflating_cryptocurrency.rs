// Copyright 2017 The Exonum Team
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

use exonum::blockchain::{ApiContext, Schema as CoreSchema, Service, Transaction, TransactionSet,
                         ExecutionResult};
use exonum::messages::{Message, RawTransaction};
use exonum::storage::{Fork, MapIndex, Snapshot};
use exonum::crypto::{Hash, PublicKey};
use exonum::encoding;
use exonum::api::ext::{ApiError, Endpoint, Context, ServiceApi, Spec, Visibility};
use exonum::api::iron::{Handler, IronAdapter};
use exonum::helpers::Height;

// // // // // // // // // // CONSTANTS // // // // // // // // // //

const SERVICE_ID: u16 = 1;

/// Initial balance of newly created wallet.
pub const INIT_BALANCE: u64 = 0;

// // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

encoding_struct! {
    struct Wallet {
        pub_key: &PublicKey,
        name: &str,
        balance: u64,
        last_update_height: u64,
    }
}

impl Wallet {
    pub fn actual_balance(&self, height: Height) -> u64 {
        assert!(height.0 >= self.last_update_height());
        self.balance() + height.0 - self.last_update_height()
    }

    pub fn increase(self, amount: u64, height: Height) -> Self {
        let balance = self.actual_balance(height) + amount;
        Self::new(self.pub_key(), self.name(), balance, height.0)
    }

    pub fn decrease(self, amount: u64, height: Height) -> Self {
        let balance = self.actual_balance(height) - amount;
        Self::new(self.pub_key(), self.name(), balance, height.0)
    }
}

// // // // // // // // // // DATA LAYOUT // // // // // // // // // //

pub struct CurrencySchema<S> {
    view: S,
}

impl<S: AsRef<Snapshot>> CurrencySchema<S> {
    pub fn new(view: S) -> Self {
        CurrencySchema { view }
    }

    pub fn wallets(&self) -> MapIndex<&Snapshot, PublicKey, Wallet> {
        MapIndex::new("cryptocurrency.wallets", self.view.as_ref())
    }

    /// Get a separate wallet from the storage.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }
}

impl<'a> CurrencySchema<&'a mut Fork> {
    pub fn wallets_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Wallet> {
        MapIndex::new("cryptocurrency.wallets", self.view)
    }
}

// // // // // // // // // // TRANSACTIONS // // // // // // // // // //

transactions! {
    CurrencyTransactions {
        const SERVICE_ID = SERVICE_ID;

        /// Create a new wallet.
        struct TxCreateWallet {
            pub_key: &PublicKey,
            name: &str,
        }

        /// Transfer coins between the wallets.
        struct TxTransfer {
            from: &PublicKey,
            to: &PublicKey,
            amount: u64,
            seed: u64,
        }
    }
}

// // // // // // // // // // CONTRACTS // // // // // // // // // //

impl Transaction for TxCreateWallet {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) -> ExecutionResult {
        let height = CoreSchema::new(&view).height();
        let mut schema = CurrencySchema { view };
        if schema.wallet(self.pub_key()).is_none() {
            let wallet = Wallet::new(self.pub_key(), self.name(), INIT_BALANCE, height.0);
            schema.wallets_mut().put(self.pub_key(), wallet);
        }
        Ok(())
    }
}

impl Transaction for TxTransfer {
    fn verify(&self) -> bool {
        (*self.from() != *self.to()) && self.verify_signature(self.from())
    }

    fn execute(&self, view: &mut Fork) -> ExecutionResult {
        let height = CoreSchema::new(&view).height();
        let mut schema = CurrencySchema { view };
        let sender = schema.wallet(self.from());
        let receiver = schema.wallet(self.to());
        if let (Some(sender), Some(receiver)) = (sender, receiver) {
            let amount = self.amount();
            if sender.actual_balance(height) >= amount {
                let sender = sender.decrease(amount, height);
                let receiver = receiver.increase(amount, height);
                let mut wallets = schema.wallets_mut();
                wallets.put(self.from(), sender);
                wallets.put(self.to(), receiver);
            }
        }
        Ok(())
    }
}

// // // // // // // // // // REST API // // // // // // // // // //

const BALANCE_SPEC: Spec = Spec {
    id: "balance",
    visibility: Visibility::Public,
};

fn balance(ctx: &Context, pubkey: PublicKey) -> Result<u64, ApiError> {
    let snapshot = ctx.snapshot();
    let schema = CurrencySchema::new(&snapshot);
    let wallet = schema.wallet(&pubkey).ok_or(ApiError::NotFound)?;

    let height = CoreSchema::new(&snapshot).height();
    Ok(wallet.actual_balance(height))
}

// // // // // // // // // // SERVICE DECLARATION // // // // // // // // // //

/// Define the service.
pub struct CurrencyService;

/// Implement a `Service` trait for the service.
impl Service for CurrencyService {
    fn service_name(&self) -> &str {
        "cryptocurrency"
    }

    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        Vec::new()
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let tx = CurrencyTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut api = ServiceApi::new();
        api.set_transactions::<CurrencyTransactions>();
        api.insert(BALANCE_SPEC, Endpoint::new(balance));
        Some(IronAdapter::with_context(ctx).create_handler(api))
    }
}
