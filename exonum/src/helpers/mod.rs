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

pub use self::types::{Height, Milliseconds, Round, ValidatorId, ZeroizeOnDrop};

pub mod config;
pub mod user_agent;

use env_logger::Builder;
use log::SetLoggerError;

use std::path::{Component, Path, PathBuf};

use crate::{
    api::manager::UpdateEndpoints,
    blockchain::{
        config::{GenesisConfig, GenesisConfigBuilder},
        ConsensusConfig, InstanceCollection, Schema, ValidatorKeys,
    },
    crypto::gen_keypair,
    exonum_merkledb::Fork,
    node::{ConnectListConfig, NodeConfig},
    runtime::rust::RustRuntime,
};
use exonum_keys::Keys;
use futures::sync::mpsc;

mod types;

/// Performs the logger initialization.
pub fn init_logger() -> Result<(), SetLoggerError> {
    Builder::from_default_env()
        .default_format_timestamp_nanos(true)
        .try_init()
}

/// Generates testnet configuration.
pub fn generate_testnet_config(count: u16, start_port: u16) -> Vec<NodeConfig> {
    let keys: (Vec<_>) = (0..count as usize)
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
            services_configs: Default::default(),
            database: Default::default(),
            thread_pool_size: Default::default(),
            master_key_path: "master.key.toml".into(),
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

/// This routine is adapted from the *old* Path's `path_relative_from`
/// function, which works differently from the new `relative_from` function.
/// In particular, this handles the case on unix where both paths are
/// absolute but with only the root as the common directory.
///
/// @see https://github.com/rust-lang/rust/blob/e1d0de82cc40b666b88d4a6d2c9dcbc81d7ed27f/src/librustc_back/rpath.rs#L116-L158
pub fn path_relative_from(path: impl AsRef<Path>, base: impl AsRef<Path>) -> Option<PathBuf> {
    let path = path.as_ref();
    let base = base.as_ref();

    if path.is_absolute() != base.is_absolute() {
        if path.is_absolute() {
            Some(PathBuf::from(path))
        } else {
            None
        }
    } else {
        let mut ita = path.components();
        let mut itb = base.components();
        let mut comps: Vec<Component<'_>> = vec![];
        loop {
            match (ita.next(), itb.next()) {
                (None, None) => break,
                (Some(a), None) => {
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
                (None, _) => comps.push(Component::ParentDir),
                (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                (Some(a), Some(b)) if b == Component::CurDir => comps.push(a),
                (Some(_), Some(b)) if b == Component::ParentDir => return None,
                (Some(a), Some(_)) => {
                    comps.push(Component::ParentDir);
                    for _ in itb {
                        comps.push(Component::ParentDir);
                    }
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
            }
        }
        Some(comps.iter().map(|c| c.as_os_str()).collect())
    }
}

/// Clears consensus messages cache.
///
/// Used in `exonum-cli` to implement `clear-cache` maintenance action.
pub fn clear_consensus_messages_cache(fork: &Fork) {
    Schema::new(fork).consensus_messages_cache().clear();
}

/// Returns sufficient number of votes for the given validators number.
pub fn byzantine_quorum(total: usize) -> usize {
    total * 2 / 3 + 1
}

/// Creates and initializes RustRuntime and GenesisConfig with information from collection of InstanceCollection.
pub fn create_rust_runtime_and_genesis_config(
    api_notifier: mpsc::Sender<UpdateEndpoints>,
    consensus_config: ConsensusConfig,
    instances: impl IntoIterator<Item = InstanceCollection>,
) -> (RustRuntime, GenesisConfig) {
    let mut rust_runtime = RustRuntime::new(api_notifier);
    let mut config_builder = GenesisConfigBuilder::with_consensus_config(consensus_config);

    for instance_collection in instances {
        let (factory, config) = instance_collection.into();
        rust_runtime = rust_runtime.with_available_service(factory);
        config_builder = config_builder.with_service(config);
    }

    (rust_runtime, config_builder.build())
}

#[test]
fn test_path_relative_from() {
    let cases = vec![
        ("/b", "/c", Some("../b".into())),
        ("/a/b", "/a/c", Some("../b".into())),
        ("/a", "/a/c", Some("../".into())),
        ("/a/b/c", "/a/b", Some("c".into())),
        ("/a/b/c", "/a/b/d", Some("../c".into())),
        ("./a/b", "./a/c", Some("../b".into())),
    ];
    for c in cases {
        assert_eq!(path_relative_from(c.0, c.1), c.2);
    }
}
