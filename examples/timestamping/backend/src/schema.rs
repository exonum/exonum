// Copyright 2018 The Exonum Team
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
use exonum::{
    crypto::Hash, storage::{Fork, ProofMapIndex, Snapshot},
};

encoding_struct! {
    /// Stores content's hash and some metadata about it.
    struct Timestamp {
        /// Hash of the content.
        content_hash: &Hash,

        /// Additional metadata.
        metadata: &str,
    }
}

encoding_struct! {
    /// Timestamp entry
    struct TimestampEntry {
        /// Timestamp data.
        timestamp: Timestamp,

        /// Hash of transaction.
        tx_hash: &Hash,

        /// Timestamp time.
        time: DateTime<Utc>,
    }
}

#[derive(Debug)]
pub struct Schema<T> {
    view: T,
}

/// Timestamping information schema.
impl<T> Schema<T> {
    pub fn new(snapshot: T) -> Self {
        Schema { view: snapshot }
    }
}

impl<T> Schema<T>
where
    T: AsRef<Snapshot>,
{
    pub fn timestamps(&self) -> ProofMapIndex<&T, Hash, TimestampEntry> {
        ProofMapIndex::new("timestamping.timestamps", &self.view)
    }

    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.timestamps().merkle_root()]
    }
}

impl<'a> Schema<&'a mut Fork> {
    pub fn timestamps_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, TimestampEntry> {
        ProofMapIndex::new("timestamping.timestamps", &mut self.view)
    }

    pub fn add_timestamp(&mut self, timestamp_entry: TimestampEntry) {
        let timestamp = timestamp_entry.timestamp();
        let content_hash = timestamp.content_hash();

        // Check that timestamp with given content_hash does not exist.
        if self.timestamps().contains(content_hash) {
            return;
        }

        // Add timestamp
        self.timestamps_mut().put(content_hash, timestamp_entry);
    }
}
