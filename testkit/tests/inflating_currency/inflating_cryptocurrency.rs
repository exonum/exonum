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

use exonum::{
    blockchain::Schema as CoreSchema,
    crypto::{Hash, PublicKey},
    helpers::Height,
    runtime::{
        api::{self, ServiceApiBuilder},
        rust::{Service, TransactionContext},
        InstanceDescriptor, InstanceId,
    },
};
use exonum_derive::{exonum_service, BinaryValue, IntoExecutionError, ObjectHash, ServiceFactory};
use exonum_merkledb::{IndexAccess, MapIndex, Snapshot};
use exonum_proto_derive::ProtobufConvert;
use serde_derive::{Deserialize, Serialize};

use super::proto;

// // // // // // // // // // CONSTANTS // // // // // // // // // //

pub const SERVICE_ID: InstanceId = 55;
pub const SERVICE_NAME: &str = "cryptocurrency";

/// Initial balance of a newly created wallet.
pub const INIT_BALANCE: u64 = 0;

// // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
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

pub struct CurrencySchema<S: IndexAccess> {
    view: S,
}

impl<T: IndexAccess> CurrencySchema<T> {
    /// Creates a new schema instance.
    pub fn new(view: T) -> Self {
        CurrencySchema { view }
    }

    /// Returns an immutable version of the wallets table.
    pub fn wallets(&self) -> MapIndex<T, PublicKey, Wallet> {
        MapIndex::new("cryptocurrency.wallets", self.view.clone())
    }

    /// Gets a specific wallet from the storage.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }
}

// // // // // // // // // // TRANSACTIONS // // // // // // // // // //

/// Create a new wallet.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[exonum(pb = "proto::TxCreateWallet")]
pub struct TxCreateWallet {
    pub name: String,
}

/// Transfer coins between the wallets.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[exonum(pb = "proto::TxTransfer")]
pub struct TxTransfer {
    pub to: PublicKey,
    pub amount: u64,
    pub seed: u64,
}

// // // // // // // // // // CONTRACTS // // // // // // // // // //

#[derive(Debug, IntoExecutionError)]
pub enum Error {
    /// Dummy
    Foo = 0,
}

#[exonum_service]
pub trait CurrencyInterface {
    /// Apply logic to the storage when executing the transaction.
    fn create_wallet(&self, context: TransactionContext, arg: TxCreateWallet) -> Result<(), Error>;
    /// Retrieve two wallets to apply the transfer. Check the sender's
    /// balance and apply changes to the balances of the wallets.
    fn transfer(&self, context: TransactionContext, arg: TxTransfer) -> Result<(), Error>;
}

impl CurrencyInterface for CurrencyService {
    fn create_wallet(&self, context: TransactionContext, arg: TxCreateWallet) -> Result<(), Error> {
        let author = context.caller().author().unwrap();

        let height = CoreSchema::new(context.fork()).height();
        let schema = CurrencySchema::new(context.fork());
        if schema.wallet(&author).is_none() {
            let wallet = Wallet::new(&author, &arg.name, INIT_BALANCE, height.0);
            schema.wallets().put(&author, wallet);
        }
        Ok(())
    }

    fn transfer(&self, context: TransactionContext, arg: TxTransfer) -> Result<(), Error> {
        let author = context.caller().author().unwrap();

        if author == arg.to {
            return Err(Error::Foo);
        }
        let view = context.fork();
        let height = CoreSchema::new(view).height();
        let schema = CurrencySchema { view };
        let sender = schema.wallet(&author);
        let receiver = schema.wallet(&arg.to);
        if let (Some(sender), Some(receiver)) = (sender, receiver) {
            let amount = arg.amount;
            if sender.actual_balance(height) >= amount {
                let sender = sender.decrease(amount, height);
                let receiver = receiver.increase(amount, height);
                let mut wallets = schema.wallets();
                wallets.put(&author, sender);
                wallets.put(&arg.to, receiver);
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
        let schema = CurrencySchema::new(snapshot);
        schema
            .wallet(&query.pub_key)
            .map(|wallet| {
                let height = CoreSchema::new(snapshot).height();
                wallet.actual_balance(height)
            })
            .ok_or_else(|| api::Error::NotFound("Wallet not found".to_owned()))
    }

    fn wire(builder: &mut ServiceApiBuilder) {
        builder.public_scope().endpoint("v1/balance", Self::balance);
    }
}

// // // // // // // // // // SERVICE DECLARATION // // // // // // // // // //

/// Define the service.
#[derive(Debug, ServiceFactory)]
#[exonum(
    artifact_name = "cryptocurrency",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto",
    implements("CurrencyInterface")
)]
pub struct CurrencyService;

/// Implement a `Service` trait for the service.
impl Service for CurrencyService {
    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        CryptocurrencyApi::wire(builder)
    }

    fn state_hash(&self, _instance: InstanceDescriptor, _snapshot: &dyn Snapshot) -> Vec<Hash> {
        vec![]
    }
}
