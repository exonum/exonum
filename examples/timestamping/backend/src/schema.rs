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

//! Timestamping database schema.

use chrono::{DateTime, Utc};
use exonum::crypto::Hash;
use exonum_merkledb::{Entry, IndexAccess, ObjectHash, ProofMapIndex};
use exonum_proto_derive::ProtobufConvert;

use crate::{proto, transactions::Config};

/// Stores content's hash and some metadata about it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ProtobufConvert, BinaryValue, ObjectHash)]
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
#[derive(Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
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
    pub fn new(timestamp: Timestamp, tx_hash: Hash, time: DateTime<Utc>) -> Self {
        Self {
            timestamp,
            tx_hash,
            time,
        }
    }
}

/// Timestamping database schema.
#[derive(Debug)]
pub struct Schema<'a, T> {
    service_name: &'a str,
    access: T,
}

impl<'a, T> Schema<'a, T> {
    /// Creates a new schema from the database view.
    pub fn new(service_name: &'a str, access: T) -> Self {
        Schema {
            service_name,
            access,
        }
    }
}

impl<'a, T> Schema<'a, T>
where
    T: IndexAccess,
{
    /// Returns the `ProofMapIndex` of timestamps.
    pub fn timestamps(&self) -> ProofMapIndex<T, Hash, TimestampEntry> {
        ProofMapIndex::new(
            [self.service_name, ".timestamps"].concat(),
            self.access.clone(),
        )
    }

    /// Returns the actual timestamping configuration
    pub fn config(&self) -> Entry<T, Config> {
        Entry::new([self.service_name, ".config"].concat(), self.access.clone())
    }

    /// Returns the state hash of the timestamping service.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.timestamps().object_hash()]
    }

    /// Adds the timestamp entry to the database.
    pub fn add_timestamp(&self, timestamp_entry: TimestampEntry) {
        let timestamp = timestamp_entry.timestamp.clone();
        let content_hash = &timestamp.content_hash;

        // Check that timestamp with given content_hash does not exist.
        if self.timestamps().contains(content_hash) {
            return;
        }

        // Add timestamp
        self.timestamps().put(content_hash, timestamp_entry);
    }
}
