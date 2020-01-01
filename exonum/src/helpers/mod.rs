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

//! Different assorted utilities.

pub use self::types::{Height, Round, ValidatorId};

// Required by `consensus-tests`. This is not a public API, since `user_agent::get` is hidden under `doc(hidden)`.
pub use self::user_agent::user_agent;

// `Milliseconds` is just `u64`, but more readable within context.
pub(crate) use self::types::Milliseconds;

use env_logger::Builder;
use log::SetLoggerError;

use crate::{blockchain::Schema, exonum_merkledb::Fork};

mod types;
mod user_agent;

/// Performs the logger initialization.
pub fn init_logger() -> Result<(), SetLoggerError> {
    Builder::from_default_env()
        .default_format_timestamp_nanos(true)
        .try_init()
}

/// Generates testnet configuration. This function needs to be public to be used in `tests`.
#[doc(hidden)]
pub fn generate_testnet_config(count: u16, start_port: u16) -> Vec<crate::node::NodeConfig> {
    use crate::{
        blockchain::{ConsensusConfig, ValidatorKeys},
        crypto::gen_keypair,
        node::{ConnectListConfig, NodeConfig},
    };
    use exonum_keys::Keys;

    let keys: Vec<_> = (0..count as usize)
        .map(|_| (gen_keypair(), gen_keypair()))
        .map(|(v, s)| Keys::from_keys(v.0, v.1, s.0, s.1))
        .collect();

    let consensus = ConsensusConfig {
        validator_keys: keys
            .iter()
            .map(|keys| ValidatorKeys {
                consensus_key: keys.consensus_pk(),
                service_key: keys.service_pk(),
            })
            .collect(),
        ..ConsensusConfig::default()
    };
    let peers = (0..keys.len())
        .map(|x| format!("127.0.0.1:{}", start_port + x as u16))
        .collect::<Vec<_>>();

    keys.into_iter()
        .enumerate()
        .map(|(idx, keys)| NodeConfig {
            listen_address: peers[idx].parse().unwrap(),
            external_address: peers[idx].clone(),
            network: Default::default(),
            consensus: consensus.clone(),
            connect_list: ConnectListConfig::from_validator_keys(&consensus.validator_keys, &peers),
            api: Default::default(),
            mempool: Default::default(),
            thread_pool_size: Default::default(),
            keys,
        })
        .collect::<Vec<_>>()
}

/// Basic trait to validate user defined input.
pub trait ValidateInput: Sized {
    /// The type returned in the event of a validate error.
    type Error;
    /// Perform parameters validation for this configuration and return error if
    /// value is inconsistent.
    fn validate(&self) -> Result<(), Self::Error>;
    /// The same as validate method, but returns the value itself as a successful result.
    fn into_validated(self) -> Result<Self, Self::Error> {
        self.validate().map(|_| self)
    }
}

/// Clears consensus messages cache.
///
/// Used in `exonum-cli` to implement `clear-cache` maintenance action.
#[doc(hidden)]
pub fn clear_consensus_messages_cache(fork: &Fork) {
    Schema::new(fork).consensus_messages_cache().clear();
}

/// Returns sufficient number of votes for the given validators number.
pub fn byzantine_quorum(total: usize) -> usize {
    total * 2 / 3 + 1
}
