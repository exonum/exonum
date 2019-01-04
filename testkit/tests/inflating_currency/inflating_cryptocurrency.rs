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

extern crate failure;
extern crate serde;
extern crate serde_json;

use exonum::{
    api,
    blockchain::{
        ExecutionError, ExecutionResult, Schema as CoreSchema, Service, Transaction,
        TransactionContext, TransactionSet,
    },
    crypto::{Hash, PublicKey, SecretKey},
    helpers::Height,
    messages::{Message, RawTransaction, Signed},
    storage::{Fork, MapIndex, Snapshot},
};

use super::proto;

// // // // // // // // // // CONSTANTS // // // // // // // // // //

const SERVICE_ID: u16 = 1;

/// Initial balance of newly created wallet.
pub const INIT_BALANCE: u64 = 0;

// // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Wallet")]
pub struct Wallet {
    pub pub_key: PublicKey,
    pub name: String,
    pub balance: u64,
    pub last_update_height: u64,
}

impl Wallet {
    pub fn new(&pub_key: &PublicKey, name: &str, balance: u64, last_update_height: u64) -> Self {
        Self {
            pub_key,
            name: name.to_owned(),
            balance,
            last_update_height,
        }
    }

    pub fn actual_balance(&self, height: Height) -> u64 {
        assert!(height.0 >= self.last_update_height);
        self.balance + height.0 - self.last_update_height
    }

    pub fn increase(self, amount: u64, height: Height) -> Self {
        let balance = self.actual_balance(height) + amount;
        Self::new(&self.pub_key, &self.name, balance, height.0)
    }

    pub fn decrease(self, amount: u64, height: Height) -> Self {
        let balance = self.actual_balance(height) - amount;
        Self::new(&self.pub_key, &self.name, balance, height.0)
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

/// Create a new wallet.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TxCreateWallet")]
pub struct TxCreateWallet {
    name: String,
}

/// Transfer coins between the wallets.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TxTransfer")]
pub struct TxTransfer {
    to: PublicKey,
    amount: u64,
    seed: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, TransactionSet)]
pub(in inflating_cryptocurrency) enum CurrencyTransactions {
    TxCreateWallet(TxCreateWallet),
    TxTransfer(TxTransfer),
}

impl TxCreateWallet {
    #[doc(hidden)]
    pub fn sign(name: &str, pk: &PublicKey, sk: &SecretKey) -> Signed<RawTransaction> {
        Message::sign_transaction(
            Self {
                name: name.to_owned(),
            },
            SERVICE_ID,
            *pk,
            sk,
        )
    }
}

impl TxTransfer {
    #[doc(hidden)]
    pub fn sign(
        &to: &PublicKey,
        amount: u64,
        seed: u64,
        pk: &PublicKey,
        sc: &SecretKey,
    ) -> Signed<RawTransaction> {
        Message::sign_transaction(Self { to, amount, seed }, SERVICE_ID, *pk, sc)
    }
}

// // // // // // // // // // CONTRACTS // // // // // // // // // //

impl Transaction for TxCreateWallet {
    /// Apply logic to the storage when executing the transaction.
    fn execute(&self, mut tc: TransactionContext) -> ExecutionResult {
        let author = tc.author();
        let view = tc.fork();
        let height = CoreSchema::new(&view).height();
        let mut schema = CurrencySchema { view };
        if schema.wallet(&author).is_none() {
            let wallet = Wallet::new(&author, &self.name, INIT_BALANCE, height.0);
            schema.wallets_mut().put(&author, wallet);
        }
        Ok(())
    }
}

impl Transaction for TxTransfer {
    /// Retrieve two wallets to apply the transfer. Check the sender's
    /// balance and apply changes to the balances of the wallets.
    fn execute(&self, mut tc: TransactionContext) -> ExecutionResult {
        let author = tc.author();
        if author == self.to {
            Err(ExecutionError::new(0))?
        }
        let view = tc.fork();
        let height = CoreSchema::new(&view).height();
        let mut schema = CurrencySchema { view };
        let sender = schema.wallet(&author);
        let receiver = schema.wallet(&self.to);
        if let (Some(sender), Some(receiver)) = (sender, receiver) {
            let amount = self.amount;
            if sender.actual_balance(height) >= amount {
                let sender = sender.decrease(amount, height);
                let receiver = receiver.increase(amount, height);
                let mut wallets = schema.wallets_mut();
                wallets.put(&author, sender);
                wallets.put(&self.to, receiver);
            }
        }
        Ok(())
    }
}

// // // // // // // // // // REST API // // // // // // // // // //

struct CryptocurrencyApi;

#[derive(Debug, Serialize, Deserialize)]
struct BalanceQuery {
    pub_key: PublicKey,
}

/// Shortcut to get data on wallets.
impl CryptocurrencyApi {
    /// Endpoint for retrieving a single wallet.
    fn balance(state: &api::ServiceApiState, query: BalanceQuery) -> api::Result<u64> {
        let snapshot = state.snapshot();
        let schema = CurrencySchema::new(&snapshot);
        schema
            .wallet(&query.pub_key)
            .map(|wallet| {
                let height = CoreSchema::new(&snapshot).height();
                wallet.actual_balance(height)
            })
            .ok_or_else(|| api::Error::NotFound("Wallet not found".to_owned()))
    }

    fn wire(builder: &mut api::ServiceApiBuilder) {
        builder.public_scope().endpoint("v1/balance", Self::balance);
    }
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

    /// Implement a method to deserialize transactions coming to the node.
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, failure::Error> {
        let tx = CurrencyTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    fn wire_api(&self, builder: &mut api::ServiceApiBuilder) {
        CryptocurrencyApi::wire(builder)
    }
}
