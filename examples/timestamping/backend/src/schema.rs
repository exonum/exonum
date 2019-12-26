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
use exonum_crypto::Hash;
use exonum_derive::{BinaryValue, FromAccess, ObjectHash};
use exonum_merkledb::{
    access::{Access, RawAccessMut},
    Entry, RawProofMapIndex,
};
use exonum_proto::ProtobufConvert;

use crate::{proto, transactions::Config};

/// Stores content's hash and some metadata about it.
#[derive(Clone, Debug, PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Timestamp")]
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
#[protobuf_convert(source = "proto::TimestampEntry", serde_pb_convert)]
#[derive(Clone, Debug)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
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
#[derive(Debug, FromAccess)]
pub struct Schema<T: Access> {
    pub config: Entry<T::Base, Config>,
    pub timestamps: RawProofMapIndex<T::Base, Hash, TimestampEntry>,
}

impl<T> Schema<T>
where
    T: Access,
    T::Base: RawAccessMut,
{
    /// Adds the timestamp entry to the database.
    pub fn add_timestamp(&mut self, timestamp_entry: TimestampEntry) {
        let timestamp = timestamp_entry.timestamp.clone();
        let content_hash = &timestamp.content_hash;

        // Check that timestamp with given content_hash does not exist.
        if self.timestamps.contains(content_hash) {
            return;
        }
        // Add the timestamp.
        self.timestamps.put(content_hash, timestamp_entry);
    }
}
