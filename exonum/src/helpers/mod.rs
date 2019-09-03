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
pub mod fabric;
pub mod user_agent;
#[macro_use]
pub mod metrics;

use env_logger::Builder;
use log::SetLoggerError;

use std::path::{Component, Path, PathBuf};

use crate::blockchain::{GenesisConfig, ValidatorKeys};
use crate::crypto::gen_keypair;
use crate::node::{ConnectListConfig, NodeConfig};
use exonum_crypto::{kx, Keys};

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
        .map(|_| (gen_keypair(), gen_keypair(), kx::gen_keypair()))
        .map(|(v, s, i)| Keys {
            consensus_pk: v.0,
            consensus_sk: v.1,
            service_pk: s.0,
            service_sk: s.1,
            identity_pk: i.0,
            identity_sk: i.1,
        })
        .collect();

    let genesis = GenesisConfig::new(keys.iter().map(|keys| ValidatorKeys {
        consensus_key: keys.consensus_pk,
        service_key: keys.service_pk,
        identity_key: keys.identity_pk,
    }));
    let peers = (0..keys.len())
        .map(|x| format!("127.0.0.1:{}", start_port + x as u16))
        .collect::<Vec<_>>();

    keys.into_iter()
        .enumerate()
        .map(|(idx, keys)| NodeConfig {
            listen_address: peers[idx].parse().unwrap(),
            external_address: peers[idx].clone(),
            network: Default::default(),
            genesis: genesis.clone(),
            connect_list: ConnectListConfig::from_validator_keys(&genesis.validator_keys, &peers),
            api: Default::default(),
            mempool: Default::default(),
            services_configs: Default::default(),
            database: Default::default(),
            thread_pool_size: Default::default(),
            master_key_path: Default::default(),
            keys,
        })
        .collect::<Vec<_>>()
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
        let mut comps: Vec<Component> = vec![];
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
