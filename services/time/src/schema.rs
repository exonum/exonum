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

use chrono::{DateTime, Utc};

use exonum::crypto::{Hash, PublicKey};
use exonum_merkledb::{Entry, IndexAccess, ObjectHash, ProofMapIndex};

/// `Exonum-time` service database schema.
#[derive(Debug)]
pub struct TimeSchema<T> {
    access: T,
}

impl<T: IndexAccess> TimeSchema<T> {
    /// Constructs schema for the given `snapshot`.
    pub fn new(access: T) -> Self {
        TimeSchema { access }
    }

    /// Returns the table that stores `DateTime` for every validator.
    pub fn validators_times(&self) -> ProofMapIndex<T, PublicKey, DateTime<Utc>> {
        ProofMapIndex::new("exonum_time.validators_times", self.access.clone())
    }

    /// Returns stored time.
    pub fn time(&self) -> Entry<T, DateTime<Utc>> {
        Entry::new("exonum_time.time", self.access.clone())
    }

    /// Returns hashes for stored tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.validators_times().object_hash(), self.time().hash()]
    }
}
