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

use exonum::{
    crypto::Hash,
    runtime::{versioning::Version, ExecutionError},
};
use exonum_derive::*;
use exonum_proto::ProtobufConvert;
use serde_derive::{Deserialize, Serialize};

use super::{proto, AsyncEventState, MigrationError};

/// State of a migration.
#[derive(Debug, Clone)]
#[derive(ProtobufConvert, BinaryValue)]
#[derive(Serialize, Deserialize)]
#[protobuf_convert(source = "proto::MigrationState")]
pub struct MigrationState {
    /// Migration process state.
    #[serde(rename = "state")]
    pub inner: AsyncEventState,

    /// Current artifact data version.
    #[protobuf_convert(with = "exonum::helpers::pb_version")]
    pub version: Version,

    /// Reference state hash. Equals to the first obtained local migration state hash.
    /// For a good scenario, all the hashes should be equal between each other.
    /// For the bad scenario, at least one node obtains the different hash and that's enough
    /// to consider migration failed.
    #[protobuf_convert(with = "exonum::helpers::pb_optional_hash")]
    #[serde(skip)]
    pub(crate) reference_state_hash: Option<Hash>,
}

impl MigrationState {
    /// Creates a new `MigrationState` object.
    pub fn new(inner: AsyncEventState, version: Version) -> Self {
        Self {
            inner,
            version,
            reference_state_hash: None,
        }
    }

    /// Adds a new state hash to the migration state.
    /// If this is a first hash, the `expected_hash` value will be initialized.
    /// Otherwise, provided hash will be compared to `expected_hash`.
    pub fn add_state_hash(&mut self, state_hash: Hash) -> Result<(), ExecutionError> {
        if let Some(reference_state_hash) = self.reference_state_hash {
            // We already have an expected hash, so we compare a new one against it.
            if reference_state_hash == state_hash {
                // Hashes match, that's OK.
            } else {
                // Hashes do not match, report an error.
                return Err(MigrationError::StateHashDivergence.into());
            }
        } else {
            // No state hash yet, initialize it with the provided value.
            self.reference_state_hash = Some(state_hash);
        }
        Ok(())
    }

    /// Checks whether migration is failed.
    pub fn is_failed(&self) -> bool {
        self.inner.is_failed()
    }

    /// Checks whether migration is pending.
    pub fn is_pending(&self) -> bool {
        self.inner.is_pending()
    }

    /// Updates migration state to the new state and artifact.
    pub fn update(&mut self, new_state: AsyncEventState, version: Version) {
        self.inner = new_state;
        self.version = version;
    }

    /// Marks migration as failed.
    pub fn fail(&mut self, new_state: AsyncEventState) {
        debug_assert!(new_state.is_failed());

        self.inner = new_state;
    }

    /// Returns the reference state hash.
    #[doc(hidden)] // Public for tests.
    pub fn reference_state_hash(&self) -> &Option<Hash> {
        &self.reference_state_hash
    }
}
