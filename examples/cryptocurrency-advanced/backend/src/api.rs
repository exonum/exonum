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

//! Cryptocurrency API.

use exonum::{
    api::{self, ServiceApiBuilder, ServiceApiState},
    blockchain::{self, BlockProof, Transaction, TransactionSet}, crypto::{Hash, PublicKey},
    helpers::Height, node::TransactionSend, storage::{ListProof, MapProof},
};

use transactions::WalletTransactions;
use wallet::Wallet;
use {CurrencySchema, CRYPTOCURRENCY_SERVICE_ID};

/// The structure describes the query parameters for the `get_wallet` endpoint.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WalletQuery {
    /// Public key of the queried wallet.
    pub pub_key: PublicKey,
}

/// The structure returned by the REST API.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResponse {
    /// Hash of the transaction.
    pub tx_hash: Hash,
}

/// Proof of existence for specific wallet.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletProof {
    /// Proof to the whole database table.
    pub to_table: MapProof<Hash, Hash>,
    /// Proof to the specific wallet in this table.
    pub to_wallet: MapProof<PublicKey, Wallet>,
}

/// Wallet history.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletHistory {
    pub proof: ListProof<Hash>,
    pub transactions: Vec<WalletTransactions>,
}

/// Wallet information.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletInfo {
    pub block_proof: BlockProof,
    pub wallet_proof: WalletProof,
    pub wallet_history: Option<WalletHistory>,
}

// TODO: Add documentation. (ECR-1638)
/// Public service API description.
#[derive(Debug, Clone, Copy)]
pub struct CryptocurrencyApi;

impl CryptocurrencyApi {
    pub fn wallet_info(state: &ServiceApiState, query: WalletQuery) -> api::Result<WalletInfo> {
        let snapshot = state.snapshot();
        let general_schema = blockchain::Schema::new(&snapshot);
        let currency_schema = CurrencySchema::new(&snapshot);

        let max_height = general_schema.block_hashes_by_height().len() - 1;

        let block_proof = general_schema
            .block_and_precommits(Height(max_height))
            .unwrap();

        let to_table: MapProof<Hash, Hash> =
            general_schema.get_proof_to_service_table(CRYPTOCURRENCY_SERVICE_ID, 0);

        let to_wallet: MapProof<PublicKey, Wallet> =
            currency_schema.wallets().get_proof(query.pub_key);

        let wallet_proof = WalletProof {
            to_table,
            to_wallet,
        };

        let wallet = currency_schema.wallet(&query.pub_key);

        let wallet_history = wallet.map(|_| {
            let history = currency_schema.wallet_history(&query.pub_key);
            let proof = history.get_range_proof(0, history.len());

            let transactions: Vec<WalletTransactions> = history
                .iter()
                .map(|record| general_schema.transactions().get(&record).unwrap())
                .map(|raw| WalletTransactions::tx_from_raw(raw).unwrap())
                .collect::<Vec<_>>();

            WalletHistory {
                proof,
                transactions,
            }
        });

        Ok(WalletInfo {
            block_proof,
            wallet_proof,
            wallet_history,
        })
    }

    pub fn post_transaction(
        state: &ServiceApiState,
        query: WalletTransactions,
    ) -> api::Result<TransactionResponse> {
        let transaction: Box<Transaction> = query.into();
        let tx_hash = transaction.hash();
        state.sender().send(transaction)?;
        Ok(TransactionResponse { tx_hash })
    }

    pub fn wire(builder: &mut ServiceApiBuilder) {
        builder
            .public_scope()
            .endpoint("v1/wallets/info", Self::wallet_info)
            .endpoint_mut("v1/wallets/transaction", Self::post_transaction);
    }
}
