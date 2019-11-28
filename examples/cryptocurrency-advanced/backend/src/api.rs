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

//! Cryptocurrency API.

use exonum_merkledb::{proof_map_index::Raw, ListProof, MapProof};

use exonum::{
    blockchain::{BlockProof, IndexCoordinates, SchemaOrigin},
    crypto::{Hash, PublicKey},
    messages::{AnyTx, Verified},
    runtime::rust::api::{self, ServiceApiBuilder, ServiceApiState},
};

use crate::{wallet::Wallet, Schema};

/// Describes the query parameters for the `get_wallet` endpoint.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WalletQuery {
    /// Public key of the queried wallet.
    pub pub_key: PublicKey,
}

/// Proof of existence for specific wallet.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletProof {
    /// Proof of the whole database table.
    pub to_table: MapProof<IndexCoordinates, Hash>,
    /// Proof of the specific wallet in this table.
    pub to_wallet: MapProof<PublicKey, Wallet, Raw>,
}

/// Wallet history.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletHistory {
    /// Proof of the list of transaction hashes.
    pub proof: ListProof<Hash>,
    /// List of above transactions.
    pub transactions: Vec<Verified<AnyTx>>,
}

/// Wallet information.
#[derive(Debug, Serialize, Deserialize)]
pub struct WalletInfo {
    /// Proof of the last block.
    pub block_proof: BlockProof,
    /// Proof of the appropriate wallet.
    pub wallet_proof: WalletProof,
    /// History of the appropriate wallet.
    pub wallet_history: Option<WalletHistory>,
}

/// Public service API description.
#[derive(Debug, Clone, Copy)]
pub struct PublicApi;

impl PublicApi {
    /// Endpoint for getting a single wallet.
    pub fn wallet_info(
        self,
        state: &ServiceApiState<'_>,
        pub_key: PublicKey,
    ) -> api::Result<WalletInfo> {
        let blockchain_schema = state.data().for_core();
        let currency_schema = Schema::new(state.service_data());
        let current_height = blockchain_schema.height();

        let block_proof = blockchain_schema
            .block_and_precommits(current_height)
            .unwrap();
        let to_table = blockchain_schema
            .state_hash_aggregator()
            .get_proof(SchemaOrigin::Service(state.instance().id).coordinate_for(0));
        let to_wallet = currency_schema.wallets.get_proof(pub_key);

        let wallet_proof = WalletProof {
            to_table,
            to_wallet,
        };
        let wallet = currency_schema.wallets.get(&pub_key);

        let wallet_history = wallet.map(|_| {
            // `history` is always present for existing wallets.
            let history = currency_schema.wallet_history.get(&pub_key);
            let proof = history.get_range_proof(0..history.len());

            let transactions = history
                .iter()
                .map(|tx_hash| blockchain_schema.transactions().get(&tx_hash).unwrap())
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

    /// Wires the above endpoint to public scope of the given `ServiceApiBuilder`.
    pub fn wire(self, builder: &mut ServiceApiBuilder) {
        builder.public_scope().endpoint(
            "v1/wallets/info",
            move |state: &ServiceApiState<'_>, query: WalletQuery| {
                self.wallet_info(state, query.pub_key)
            },
        );
    }
}
