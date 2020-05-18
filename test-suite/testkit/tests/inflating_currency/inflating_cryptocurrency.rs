// Copyright 2020 The Exonum Team
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
    crypto::PublicKey,
    helpers::Height,
    runtime::{ExecutionContext, ExecutionError, InstanceId},
};
use exonum_derive::{
    exonum_interface, BinaryValue, ExecutionFail, FromAccess, ObjectHash, ServiceDispatcher,
    ServiceFactory,
};
use exonum_merkledb::{
    access::{Access, FromAccess},
    MapIndex,
};
use exonum_rust_runtime::{
    api::{self, ServiceApiBuilder, ServiceApiState},
    DefaultInstance, Service,
};
use serde_derive::{Deserialize, Serialize};

// // // // // // // // // // CONSTANTS // // // // // // // // // //

pub const SERVICE_ID: InstanceId = 55;
pub const SERVICE_NAME: &str = "cryptocurrency";

/// Initial balance of a newly created wallet.
pub const INIT_BALANCE: u64 = 0;

// // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
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

#[derive(FromAccess)]
pub(crate) struct CurrencySchema<T: Access> {
    pub wallets: MapIndex<T::Base, PublicKey, Wallet>,
}

impl<T: Access> CurrencySchema<T> {
    pub fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }

    /// Gets a specific wallet from the storage.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets.get(pub_key)
    }
}

// // // // // // // // // // TRANSACTIONS // // // // // // // // // //

/// Create a new wallet.
#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
pub struct CreateWallet {
    pub name: String,
}

impl CreateWallet {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

/// Transfer coins between the wallets.
#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[binary_value(codec = "bincode")]
pub struct Transfer {
    pub to: PublicKey,
    pub amount: u64,
    pub seed: u64,
}

// // // // // // // // // // CONTRACTS // // // // // // // // // //

#[derive(Debug, ExecutionFail)]
pub enum Error {
    /// Sender and receiver of the transfer are the same.
    SenderSameAsReceiver = 0,
}

#[exonum_interface(auto_ids)]
pub trait CurrencyInterface<Ctx> {
    type Output;
    /// Apply logic to the storage when executing the transaction.
    fn create_wallet(&self, ctx: Ctx, arg: CreateWallet) -> Self::Output;
    /// Retrieve two wallets to apply the transfer. Check the sender's
    /// balance and apply changes to the balances of the wallets.
    fn transfer(&self, ctx: Ctx, arg: Transfer) -> Self::Output;
}

impl CurrencyInterface<ExecutionContext<'_>> for CurrencyService {
    type Output = Result<(), ExecutionError>;

    fn create_wallet(&self, ctx: ExecutionContext<'_>, arg: CreateWallet) -> Self::Output {
        let author = ctx.caller().author().unwrap();

        let height = ctx.data().for_core().height();
        let mut schema = CurrencySchema::new(ctx.service_data());
        if schema.wallet(&author).is_none() {
            let wallet = Wallet::new(&author, &arg.name, INIT_BALANCE, height.0);
            schema.wallets.put(&author, wallet);
        }
        Ok(())
    }

    fn transfer(&self, ctx: ExecutionContext<'_>, arg: Transfer) -> Self::Output {
        let author = ctx.caller().author().unwrap();
        if author == arg.to {
            return Err(Error::SenderSameAsReceiver.into());
        }

        let height = ctx.data().for_core().height();
        let mut schema = CurrencySchema::new(ctx.service_data());
        let sender = schema.wallet(&author);
        let receiver = schema.wallet(&arg.to);
        if let (Some(sender), Some(receiver)) = (sender, receiver) {
            let amount = arg.amount;
            if sender.actual_balance(height) >= amount {
                let sender = sender.decrease(amount, height);
                let receiver = receiver.increase(amount, height);
                schema.wallets.put(&author, sender);
                schema.wallets.put(&arg.to, receiver);
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
    async fn balance(state: ServiceApiState, query: BalanceQuery) -> api::Result<u64> {
        let snapshot = state.data();
        let schema = CurrencySchema::new(snapshot.for_executing_service());
        schema
            .wallet(&query.pub_key)
            .map(|wallet| {
                let height = snapshot.for_core().height();
                wallet.actual_balance(height)
            })
            .ok_or_else(|| api::Error::not_found().title("Wallet not found"))
    }

    fn wire(builder: &mut ServiceApiBuilder) {
        builder.public_scope().endpoint("v1/balance", Self::balance);
    }
}

// // // // // // // // // // SERVICE DECLARATION // // // // // // // // // //

/// Define the service.
#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "cryptocurrency", artifact_version = "1.0.0")]
#[service_dispatcher(implements("CurrencyInterface"))]
pub struct CurrencyService;

/// Implement a `Service` trait for the service.
impl Service for CurrencyService {
    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        CryptocurrencyApi::wire(builder)
    }
}

impl DefaultInstance for CurrencyService {
    const INSTANCE_ID: u32 = SERVICE_ID;
    const INSTANCE_NAME: &'static str = SERVICE_NAME;
}
