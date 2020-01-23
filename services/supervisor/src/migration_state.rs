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

use exonum::{crypto::Hash, runtime::ExecutionError};
use exonum_derive::*;
use exonum_proto::ProtobufConvert;

use super::{proto, AsyncEventState, MigrationError};

#[derive(Debug)]
#[derive(ProtobufConvert, BinaryValue)]
#[protobuf_convert(source = "proto::MigrationState")]
pub(crate) struct MigrationState {
    /// Migration process state.
    pub inner: AsyncEventState,
    /// Expected state hash. Equals to the first obtained local migration state hash.
    /// For a good scenario, all the hashes should be equal between each other.
    /// For the bad scenario, at least one node obtains the different hash and that's enough
    /// to consider migration failed.
    #[protobuf_convert(with = "pb_expected_state_hash")]
    pub expected_state_hash: Option<Hash>,
}

impl MigrationState {
    pub fn new(inner: AsyncEventState) -> Self {
        Self {
            inner,
            expected_state_hash: None,
        }
    }

    pub fn add_state_hash(&mut self, state_hash: Hash) -> Result<(), ExecutionError> {
        if let Some(expected_hash) = self.expected_state_hash {
            // We already have an expected hash, so we compare a new one against it.
            if expected_hash == state_hash {
                // Hashes match, that's OK.
            } else {
                // Hashes do not match, report an error.
                return Err(MigrationError::StateHashDivergence.into());
            }
        } else {
            // No state hash yet, initialize it with the provided value.
            self.expected_state_hash = Some(state_hash);
        }
        Ok(())
    }

    pub fn is_failed(&self) -> bool {
        self.inner.is_failed()
    }

    pub fn update(&mut self, new_state: AsyncEventState) {
        self.inner = new_state;
    }
}

mod pb_expected_state_hash {
    use super::*;
    use exonum::crypto::proto as crypto_proto;

    pub fn from_pb(pb: crypto_proto::Hash) -> Result<Option<Hash>, failure::Error> {
        let hash = Hash::from_pb(pb)?;

        let result = if hash != Hash::zero() {
            Some(hash)
        } else {
            None
        };

        Ok(result)
    }

    pub fn to_pb(value: &Option<Hash>) -> crypto_proto::Hash {
        if let Some(value) = value {
            Hash::to_pb(value)
        } else {
            Hash::to_pb(&Hash::zero())
        }
    }
}
