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

extern crate bodyparser;
extern crate iron;
extern crate router;
extern crate serde;
extern crate serde_json;

use self::iron::headers::ContentType;
use self::iron::prelude::*;
use self::iron::status::Status;
use self::iron::{Handler, IronError};
use self::router::Router;
use exonum::api::{Api, ApiError};
use exonum::blockchain::{ApiContext, Blockchain, ExecutionResult, Schema as CoreSchema, Service,
                         Transaction, TransactionSet};
use exonum::crypto::{Hash, PublicKey};
use exonum::encoding;
use exonum::encoding::serialize::FromHex;
use exonum::helpers::Height;
use exonum::messages::{Message, RawTransaction};
use exonum::node::{ApiSender, TransactionSend};
use exonum::storage::{Fork, MapIndex, Snapshot};

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
    pub(in inflating_cryptocurrency) CurrencyTransactions {
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
    /// Verify integrity of the transaction by checking the transaction
    /// signature.
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    /// Apply logic to the storage when executing the transaction.
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
    /// Check if the sender is not the receiver. Check correctness of the
    /// sender's signature.
    fn verify(&self) -> bool {
        (*self.from() != *self.to()) && self.verify_signature(self.from())
    }

    /// Retrieve two wallets to apply the transfer. Check the sender's
    /// balance and apply changes to the balances of the wallets.
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

#[derive(Clone)]
struct CryptocurrencyApi {
    channel: ApiSender,
    blockchain: Blockchain,
}

/// The structure returned by the REST API.
#[derive(Serialize, Deserialize)]
pub struct TransactionResponse {
    pub tx_hash: Hash,
}

/// Shortcut to get data on wallets.
impl CryptocurrencyApi {
    fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        let view = self.blockchain.snapshot();
        let schema = CurrencySchema::new(view);
        schema.wallet(pub_key)
    }

    /// Endpoint for transactions.
    fn post_transaction(&self, req: &mut Request) -> IronResult<Response> {
        match req.get::<bodyparser::Struct<CurrencyTransactions>>() {
            Ok(Some(transaction)) => {
                let transaction: Box<Transaction> = transaction.into();
                let tx_hash = transaction.hash();
                self.channel.send(transaction).map_err(ApiError::from)?;
                let json = TransactionResponse { tx_hash };
                self.ok_response(&serde_json::to_value(&json).unwrap())
            }
            Ok(None) => Err(ApiError::BadRequest("Empty request body".into()))?,
            Err(e) => Err(ApiError::BadRequest(e.to_string()))?,
        }
    }

    /// Endpoint for retrieving a single wallet.
    fn balance(&self, req: &mut Request) -> IronResult<Response> {
        use self::iron::modifiers::Header;

        let path = req.url.path();
        let wallet_key = path.last().unwrap();
        let public_key = PublicKey::from_hex(wallet_key).map_err(|e| {
            IronError::new(
                e,
                (
                    Status::BadRequest,
                    Header(ContentType::json()),
                    "\"Invalid request param: `pub_key`\"",
                ),
            )
        })?;
        if let Some(wallet) = self.wallet(&public_key) {
            let height = CoreSchema::new(self.blockchain.snapshot()).height();
            self.ok_response(&serde_json::to_value(wallet.actual_balance(height)).unwrap())
        } else {
            self.not_found_response(&serde_json::to_value("Wallet not found").unwrap())
        }
    }
}

impl Api for CryptocurrencyApi {
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let post_transaction = move |req: &mut Request| self_.post_transaction(req);
        let self_ = self.clone();
        let balance = move |req: &mut Request| self_.balance(req);

        // Bind the transaction handler to a specific route.
        router.post(
            "/v1/wallets/transaction",
            post_transaction,
            "post_transaction",
        );
        router.get("/v1/balance/:pub_key", balance, "balance");
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
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let tx = CurrencyTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    /// Create a REST `Handler` to process web requests to the node.
    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = CryptocurrencyApi {
            channel: ctx.node_channel().clone(),
            blockchain: ctx.blockchain().clone(),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }
}
