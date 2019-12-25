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

//! Module with available running modes for Supervisor.
//!
//! Currently two modes are available:
//! - Simple mode. Within simple mode, the amount of checks performed by the Supervisor
//!   is reduced to the minimum: the only requirement is that every request is sent
//!   by the validator.
//! - Decentralized mode. Within decentralized mode, deploy requests
//!   and config proposals should be approved by at least (2/3+1) validators.

use serde_derive::{Deserialize, Serialize};

use exonum::helpers::byzantine_quorum;
use exonum_crypto::Hash;
use exonum_merkledb::access::Access;
use exonum_proto::ProtobufConvert;
use failure::{self, format_err};

use std::str::FromStr;

use super::{multisig::MultisigIndex, proto, DeployRequest};

/// Supervisor operating mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Simple supervisor mode: to deploy service one have to send
    /// one request to any of the validators.
    Simple,
    /// Decentralized supervisor mode: to deploy service a request should be
    /// sent to **every** validator before it will be executed.
    /// For configs, a byzantine majority of validators should vote for it.
    Decentralized,
}

impl ProtobufConvert for Mode {
    type ProtoStruct = proto::SupervisorMode;

    fn to_pb(&self) -> Self::ProtoStruct {
        match self {
            Mode::Simple => proto::SupervisorMode::SIMPLE,
            Mode::Decentralized => proto::SupervisorMode::DECENTRALIZED,
        }
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let result = match pb {
            proto::SupervisorMode::SIMPLE => Mode::Simple,
            proto::SupervisorMode::DECENTRALIZED => Mode::Decentralized,
        };

        Ok(result)
    }
}

impl Mode {
    /// Checks whether deploy should be performed within the network.
    pub fn deploy_approved<T: Access>(
        self,
        deploy: &DeployRequest,
        deploy_requests: &MultisigIndex<T, DeployRequest>,
        validators: usize,
    ) -> bool {
        match self {
            Mode::Simple => {
                // For simple supervisor request from 1 validator is enough.
                deploy_requests.confirmations(deploy) >= 1
            }
            Mode::Decentralized => {
                // Approve deploy in case 2/3+1 validators confirmed it.
                let confirmations = deploy_requests.confirmations(&deploy);
                confirmations >= byzantine_quorum(validators)
            }
        }
    }

    /// Checks whether config can be applied for the network.
    pub fn config_approved<T: Access>(
        self,
        config_hash: &Hash,
        config_confirms: &MultisigIndex<T, Hash>,
        validators: usize,
    ) -> bool {
        match self {
            Mode::Simple => {
                // For simple supervisor one confirmation (from us) is enough.
                config_confirms.confirmations(&config_hash) >= 1
            }
            Mode::Decentralized => {
                // Apply pending config in case 2/3+1 validators voted for it.
                let confirmations = config_confirms.confirmations(&config_hash);
                confirmations >= byzantine_quorum(validators)
            }
        }
    }
}

impl FromStr for Mode {
    type Err = failure::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "simple" => Ok(Mode::Simple),
            "decentralized" => Ok(Mode::Decentralized),
            _ => Err(format_err!(
                "Invalid supervisor mode: {}. Could be 'simple' or 'decentralized'",
                input
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Mode;
    use std::str::FromStr;

    #[test]
    fn simple_mode_from_str() {
        let input = "simple";
        let mode = Mode::from_str(input).unwrap();
        assert_eq!(mode, Mode::Simple);
    }

    #[test]
    fn decentralized_mode_from_str() {
        let input = "decentralized";
        let mode = Mode::from_str(input).unwrap();
        assert_eq!(mode, Mode::Decentralized);
    }

    #[test]
    fn invalid_mode_from_str() {
        let input = "invalid_mode";
        let err = Mode::from_str(input).unwrap_err();
        assert!(err.to_string().contains("Invalid supervisor mode"));
    }
}
