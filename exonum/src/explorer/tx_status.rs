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

use serde::{Serialize, Serializer, Deserializer, Deserialize};

use blockchain::{TransactionError, TransactionErrorType, TransactionResult};

/// Transaction execution status. Simplified version of `TransactionResult`.
#[serde(tag = "type", rename_all = "kebab-case")]
#[derive(Debug, Serialize, Deserialize)]
enum TxStatus<'a> {
    Success,
    Panic { description: &'a str },
    Error { code: u8, description: &'a str },
}

impl<'a> From<&'a TransactionResult> for TxStatus<'a> {
    fn from(result: &'a TransactionResult) -> TxStatus<'a> {
        use self::TransactionErrorType::*;

        match *result {
            Ok(()) => TxStatus::Success,
            Err(ref e) => {
                let description = e.description().unwrap_or_default();
                match e.error_type() {
                    Panic => TxStatus::Panic { description },
                    Code(code) => TxStatus::Error { code, description },
                }
            }
        }
    }
}

impl<'a> From<TxStatus<'a>> for TransactionResult {
    fn from(status: TxStatus<'a>) -> TransactionResult {
        fn to_option(s: &str) -> Option<String> {
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        };

        match status {
            TxStatus::Success => Ok(()),
            TxStatus::Panic { description } => {
                Err(TransactionError::panic(to_option(description)))
            }
            TxStatus::Error { code, description } => {
                Err(TransactionError::code(code, to_option(description)))
            }
        }
    }
}

pub fn serialize<S>(result: &TransactionResult, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    TxStatus::from(result).serialize(serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<TransactionResult, D::Error>
    where D: Deserializer<'de>
{
    let tx_status = TxStatus::deserialize(deserializer)?;
    Ok(TransactionResult::from(tx_status))
}