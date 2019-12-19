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

//! `serde` methods for `ExecutionError`.

use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

use super::{execution_result::ExecutionStatus, ExecutionError};

pub fn serialize<S>(inner: &ExecutionError, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    ExecutionStatus::from(Err(inner)).serialize(serializer)
}

pub fn deserialize<'a, D>(deserializer: D) -> Result<ExecutionError, D::Error>
where
    D: Deserializer<'a>,
{
    ExecutionStatus::deserialize(deserializer).and_then(|status| {
        status
            .into_result()
            .and_then(|res| match res {
                Err(err) => Ok(err),
                Ok(()) => Err("Not an error"),
            })
            .map_err(D::Error::custom)
    })
}
