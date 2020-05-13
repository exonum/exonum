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

//! Configuration of the cryptocurrency service.

use exonum_derive::{BinaryValue, ExecutionFail};
use exonum_proto::ProtobufConvert;

use super::proto;

/// Cryptocurrency configuration parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "proto::Config")]
pub struct Config {
    /// Initial balance of newly created wallet.
    pub init_balance: u64,
}

impl Config {
    /// Verifies that initial balance is more then zero.
    pub fn verify(&self) -> Result<(), ConfigError> {
        if self.init_balance > 0 {
            Ok(())
        } else {
            Err(ConfigError::BadBalance)
        }
    }
}

/// The enumeration represents errors that can occurred while validating config.
#[derive(Debug, ExecutionFail)]
pub enum ConfigError {
    /// Initial balance is less or equal zero.
    BadBalance = 32, // Starts from 32 to exclude intersection with core errors codes.
}
