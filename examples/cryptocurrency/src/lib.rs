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

//! Demo [Exonum][exonum] service implementing a simple cryptocurrency.
//! See [the documentation][docs] for a detailed step-by-step guide how to approach this demo,
//! and [the repository README][readme] on how to use, test, and contribute to it.
//!
//! **Note.** The service in this crate is intended for demo purposes only. It is not intended
//! for use in production.
//!
//! [exonum]: https://github.com/exonum/exonum
//! [docs]: https://exonum.com/doc/get-started/create-service
//! [readme]: https://github.com/exonum/cryptocurrency#readme

#![deny(missing_docs)]

// Import crates with necessary types into a new project.

#[macro_use]
extern crate exonum;

// Import necessary types from crates.

use exonum::blockchain::{Service, Transaction, ApiContext, ExecutionResult, TransactionSet};
use exonum::messages::{RawTransaction, Message};
use exonum::storage::{Fork, MapIndex, Snapshot};
use exonum::crypto::{Hash, PublicKey};
use exonum::encoding;
use exonum::api::ext::{ApiError, TypedEndpoint, Context, ServiceApi, Visibility};
use exonum::api::iron::{Handler, IronAdapter};

// // // // // // // // // // CONSTANTS // // // // // // // // // //

/// Service ID for the `Service` trait.
const SERVICE_ID: u16 = 1;

/// Initial balance of a newly created wallet.
const INIT_BALANCE: u64 = 100;

// // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

// Declare the data to be stored in the blockchain, namely wallets with balances.
// See [serialization docs][1] for details.
//
// [1]: https://exonum.com/doc/architecture/serialization

encoding_struct! {
    /// Wallet struct used to persist data within the service.
    struct Wallet {
        /// Public key of the wallet owner.
        pub_key: &PublicKey,
        /// Name of the wallet owner.
        name: &str,
        /// Current balance.
        balance: u64,
    }
}

/// Additional methods for managing balance of the wallet in an immutable fashion.
impl Wallet {
    /// Returns a copy of this wallet with the balance increased by the specified amount.
    pub fn increase(self, amount: u64) -> Self {
        let balance = self.balance() + amount;
        Self::new(self.pub_key(), self.name(), balance)
    }

    /// Returns a copy of this wallet with the balance decreased by the specified amount.
    pub fn decrease(self, amount: u64) -> Self {
        debug_assert!(self.balance() >= amount);
        let balance = self.balance() - amount;
        Self::new(self.pub_key(), self.name(), balance)
    }
}

// // // // // // // // // // DATA LAYOUT // // // // // // // // // //

/// Schema of the key-value storage used by the demo cryptocurrency service.
pub struct CurrencySchema<T> {
    view: T,
}

/// Declare the layout of data managed by the service. An instance of [`MapIndex`] is used
/// to keep wallets in the storage. Index values are serialized [`Wallet`] structs.
///
/// [`MapIndex`]: https://exonum.com/doc/architecture/storage#mapindex
/// [`Wallet`]: struct.Wallet.html
impl<T: AsRef<Snapshot>> CurrencySchema<T> {
    /// Creates a new schema instance.
    pub fn new(view: T) -> Self {
        CurrencySchema { view }
    }

    /// Returns an immutable version of the wallets table.
    pub fn wallets(&self) -> MapIndex<&Snapshot, PublicKey, Wallet> {
        MapIndex::new("cryptocurrency.wallets", self.view.as_ref())
    }

    /// Gets a specific wallet from the storage.
    pub fn wallet(&self, pub_key: &PublicKey) -> Option<Wallet> {
        self.wallets().get(pub_key)
    }
}

/// A mutable version of the schema with an additional method to persist wallets
/// to the storage.
impl<'a> CurrencySchema<&'a mut Fork> {
    /// Returns a mutable version of the wallets table.
    pub fn wallets_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Wallet> {
        MapIndex::new("cryptocurrency.wallets", &mut self.view)
    }
}

// // // // // // // // // // TRANSACTIONS // // // // // // // // // //

transactions! {
    CurrencyTransactions {
        const SERVICE_ID = SERVICE_ID;

        /// Transaction type for creating a new wallet.
        ///
        /// See [the `Transaction` trait implementation](#impl-Transaction) for details how
        /// `TxCreateWallet` transactions are processed.
        struct TxCreateWallet {
            /// Public key of the wallet's owner.
            pub_key: &PublicKey,
            /// UTF-8 string with the owner's name.
            name: &str,
        }

        /// Transaction type for transferring tokens between two wallets.
        ///
        /// See [the `Transaction` trait implementation](#impl-Transaction) for details how
        /// `TxTransfer` transactions are processed.
        struct TxTransfer {
            /// Public key of the sender.
            from: &PublicKey,
            /// Public key of the receiver.
            to: &PublicKey,
            /// Number of tokens to transfer from sender's account to receiver's account.
            amount: u64,
            /// Auxiliary number to guarantee [non-idempotence][idempotence] of transactions.
            ///
            /// [idempotence]: https://en.wikipedia.org/wiki/Idempotence
            seed: u64,
        }
    }
}

// // // // // // // // // // CONTRACTS // // // // // // // // // //

impl Transaction for TxCreateWallet {
    /// Verifies integrity of the transaction by checking the transaction
    /// signature.
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    /// If a wallet with the specified public key is not registered, then creates a new wallet
    /// with the specified public key and name, and an initial balance of 100.
    /// Otherwise, performs no op.
    fn execute(&self, view: &mut Fork) -> ExecutionResult {
        let mut schema = CurrencySchema::new(view);
        if schema.wallet(self.pub_key()).is_none() {
            let wallet = Wallet::new(self.pub_key(), self.name(), INIT_BALANCE);
            println!("Create the wallet: {:?}", wallet);
            schema.wallets_mut().put(self.pub_key(), wallet);
        }
        Ok(())
    }
}

impl Transaction for TxTransfer {
    /// Checks if the sender is not the receiver, and checks correctness of the
    /// sender's signature.
    fn verify(&self) -> bool {
        (*self.from() != *self.to()) && self.verify_signature(self.from())
    }

    /// Retrieves two wallets to apply the transfer; they should be previously registered
    /// with the help of [`TxCreateWallet`] transactions. Checks the sender's
    /// balance and applies changes to the balances of the wallets if the sender's balance
    /// is sufficient. Otherwise, performs no op.
    ///
    /// [`TxCreateWallet`]: struct.TxCreateWallet.html
    fn execute(&self, view: &mut Fork) -> ExecutionResult {
        let mut schema = CurrencySchema::new(view);
        let sender = schema.wallet(self.from());
        let receiver = schema.wallet(self.to());
        if let (Some(sender), Some(receiver)) = (sender, receiver) {
            let amount = self.amount();
            if sender.balance() >= amount {
                let sender = sender.decrease(amount);
                let receiver = receiver.increase(amount);
                println!("Transfer between wallets: {:?} => {:?}", sender, receiver);
                let mut wallets = schema.wallets_mut();
                wallets.put(self.from(), sender);
                wallets.put(self.to(), receiver);
            }
        }
        Ok(())
    }
}

// // // // // // // // // // REST API // // // // // // // // // //

/// Endpoint retrieving a single wallet from the database.
///
/// # Examples
///
/// ```
/// # extern crate exonum;
/// # extern crate exonum_cryptocurrency;
/// extern crate exonum_testkit;
/// #[macro_use] extern crate serde_json;
/// # use exonum::api::ext::Endpoint;
/// # use exonum_cryptocurrency::{CurrencyService, TxCreateWallet, GetWallet};
/// use exonum_testkit::TestKit;
///
/// # fn main() {
/// let mut testkit = TestKit::for_service(CurrencyService);
/// let (pubkey, key) = exonum::crypto::gen_keypair();
/// let tx = TxCreateWallet::new(&pubkey, "Alice", &key);
/// testkit.create_block_with_transaction(tx);
///
/// testkit.api().test(
///     &Endpoint::from(GetWallet),
///     json!(pubkey),
///     &json!({
///         "pub_key": pubkey,
///         "name": "Alice",
///         "balance": "100",
///     })
/// );
/// # }
/// ```
pub struct GetWallet;

impl TypedEndpoint for GetWallet {
    type Arg = PublicKey;
    type Output = Wallet;
    const ID: &'static str = "wallet";
    const VIS: Visibility = Visibility::Public;

    fn call(&self, context: &Context, pubkey: PublicKey) -> Result<Wallet, ApiError> {
        let schema = CurrencySchema::new(context.snapshot());
        schema.wallet(&pubkey).ok_or(ApiError::NotFound)
    }
}

/// Endpoint dumping all wallets from the storage.
///
/// # Examples
///
/// ```
/// # extern crate exonum;
/// # extern crate exonum_cryptocurrency;
/// #[macro_use] extern crate exonum_testkit;
/// #[macro_use] extern crate serde_json;
/// # use exonum::api::ext::Endpoint;
/// # use exonum::crypto;
/// # use exonum_cryptocurrency::{CurrencyService, TxCreateWallet, GetWallets};
/// use exonum_testkit::TestKit;
///
/// # fn main() {
/// let mut testkit = TestKit::for_service(CurrencyService);
// We need to ensure that Alice's pubkey is lexicographically lesser than Bob's
// in order to assert the output of the endpoint.
/// # let mut keys: Vec<_> = (0..2).into_iter().map(|_| crypto::gen_keypair()).collect();
/// # keys.sort_by(|&(a, _), &(b, _)| a.cmp(&b));
/// # let (alice_pubkey, alice_key) = keys.swap_remove(0);
/// # let (bob_pubkey, bob_key) = keys.swap_remove(0);
/// # assert!(alice_pubkey <= bob_pubkey);
/// let tx_alice = TxCreateWallet::new(&alice_pubkey, "Alice", &alice_key);
/// let tx_bob = TxCreateWallet::new(&bob_pubkey, "Bob", &bob_key);
/// testkit.create_block_with_transactions(txvec![tx_alice, tx_bob]);
///
/// testkit.api().test(
///     &Endpoint::from(GetWallets),
///     json!(null),
///     &json!([{
///         "pub_key": alice_pubkey,
///         "name": "Alice",
///         "balance": "100",
///     }, {
///         "pub_key": bob_pubkey,
///         "name": "Bob",
///         "balance": "100",
///     }])
/// );
/// # }
/// ```
pub struct GetWallets;

impl TypedEndpoint for GetWallets {
    type Arg = ();
    type Output = Vec<Wallet>;
    const ID: &'static str = "wallets";
    const VIS: Visibility = Visibility::Public;

    fn call(&self, ctx: &Context, _: ()) -> Result<Vec<Wallet>, ApiError> {
        let schema = CurrencySchema::new(ctx.snapshot());
        let wallets = schema.wallets();
        let wallets: Vec<_> = wallets.values().collect();
        Ok(wallets)
    }
}

/// Service API constructor.
fn create_api() -> ServiceApi {
    let mut api = ServiceApi::new();
    api.set_transactions::<CurrencyTransactions>();
    GetWallet.wire(&mut api);
    GetWallets.wire(&mut api);
    api
}

// // // // // // // // // // SERVICE DECLARATION // // // // // // // // // //

/// Demo cryptocurrency service.
///
/// See [the crate documentation](index.html) for context.
pub struct CurrencyService;

impl Service for CurrencyService {
    fn service_name(&self) -> &'static str {
        "cryptocurrency"
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    // Implement a method to deserialize transactions coming to the node.
    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let tx = CurrencyTransactions::tx_from_raw(raw)?;
        Ok(tx.into())
    }

    // Hashes for the service tables that will be included into the state hash.
    // To simplify things, we don't have [Merkelized tables][merkle] in the service storage
    // for now, so we return an empty vector.
    //
    // [merkle]: https://exonum.com/doc/architecture/storage/#merklized-indices
    fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
        vec![]
    }

    // Create a REST `Handler` to process web requests to the node.
    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        let api = create_api().public();
        Some(IronAdapter::with_context(context).create_handler(api))
    }
}
