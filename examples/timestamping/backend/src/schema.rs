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

//! Timestamping database schema.

use super::proto;
use chrono::{DateTime, Utc};
use exonum::{
    crypto::Hash,
    storage::{Fork, ProofMapIndex, Snapshot},
};

/// Stores content's hash and some metadata about it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ProtobufConvert)]
#[exonum(pb = "proto::Timestamp")]
pub struct Timestamp {
    /// Hash of the content.
    pub content_hash: Hash,

    /// Additional metadata.
    pub metadata: String,
}

impl Timestamp {
    /// Create new Timestamp.
    pub fn new(&content_hash: &Hash, metadata: &str) -> Self {
        Self {
            content_hash,
            metadata: metadata.to_owned(),
        }
    }
}

/// Timestamp entry.
#[derive(Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TimestampEntry", serde_pb_convert)]
pub struct TimestampEntry {
    /// Timestamp data.
    pub timestamp: Timestamp,

    /// Hash of transaction.
    pub tx_hash: Hash,

    /// Timestamp time.
    pub time: DateTime<Utc>,
}

impl TimestampEntry {
    /// New TimestampEntry.
    pub fn new(timestamp: Timestamp, &tx_hash: &Hash, time: DateTime<Utc>) -> Self {
        Self {
            timestamp,
            tx_hash,
            time,
        }
    }
}

/// Timestamping database schema.
#[derive(Debug)]
pub struct Schema<T> {
    view: T,
}

impl<T> Schema<T> {
    /// Creates a new schema from the database view.
    pub fn new(snapshot: T) -> Self {
        Schema { view: snapshot }
    }
}

impl<T> Schema<T>
where
    T: AsRef<dyn Snapshot>,
{
    /// Returns the `ProofMapIndex` of timestamps.
    pub fn timestamps(&self) -> ProofMapIndex<&T, Hash, TimestampEntry> {
        ProofMapIndex::new("timestamping.timestamps", &self.view)
    }

    /// Returns the state hash of the timestamping service.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.timestamps().merkle_root()]
    }
}

impl<'a> Schema<&'a mut Fork> {
    /// Returns the mutable `ProofMapIndex` of timestamps.
    pub fn timestamps_mut(&mut self) -> ProofMapIndex<&mut Fork, Hash, TimestampEntry> {
        ProofMapIndex::new("timestamping.timestamps", &mut self.view)
    }

    /// Adds the timestamp entry to the database.
    pub fn add_timestamp(&mut self, timestamp_entry: TimestampEntry) {
        let timestamp = timestamp_entry.timestamp.clone();
        let content_hash = &timestamp.content_hash;

        // Check that timestamp with given content_hash does not exist.
        if self.timestamps().contains(content_hash) {
            return;
        }

        // Add timestamp
        self.timestamps_mut().put(content_hash, timestamp_entry);
    }
}
