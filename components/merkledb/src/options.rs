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

//! Abstract settings for databases.

use rocksdb::DBCompressionType;
use serde_derive::{Deserialize, Serialize};

/// Options for the database.
///
/// These parameters apply to the underlying database of Exonum, currently `RocksDB`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DbOptions {
    /// Number of open files that can be used by the database.
    ///
    /// The underlying database opens multiple files during operation. If your system has a
    /// limit on the number of files which can be open simultaneously, you can
    /// adjust this option to match the limit. Note, that limiting the number
    /// of simultaneously open files might slow down the speed of database operation.
    ///
    /// Defaults to `None`, meaning that the number of open files is unlimited.
    pub max_open_files: Option<i32>,
    /// An option to indicate whether the system should create a database or not,
    /// if it's missing.
    ///
    /// This option applies to the cases when a node was
    /// switched off and is on again. If the database cannot be found at the
    /// indicated path and this option is switched on, a new database will be
    /// created at that path and blocks will be included therein.
    ///
    /// Defaults to `true`.
    pub create_if_missing: bool,
    /// An algorithm used for database compression.
    ///
    /// Defaults to `CompressionType::None`, meaning there is no compression.
    pub compression_type: CompressionType,

    /// No-op field for forward compatibility.
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl DbOptions {
    /// Creates a new `DbOptions` object.
    pub fn new(
        max_open_files: Option<i32>,
        create_if_missing: bool,
        compression_type: CompressionType,
    ) -> Self {
        Self {
            max_open_files,
            create_if_missing,
            compression_type,
            non_exhaustive: (),
        }
    }
}

/// Algorithms of compression for the database.
///
/// Database contents are stored in a set of blocks, each of which holds a
/// sequence of key-value pairs. Each block may be compressed before
/// being stored in a file. The following enum describes which
/// compression algorithm (if any) is used to compress a block.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CompressionType {
    Bz2,
    Lz4,
    Lz4hc,
    Snappy,
    Zlib,
    Zstd,
    None,
}

impl From<CompressionType> for DBCompressionType {
    fn from(compression_type: CompressionType) -> Self {
        match compression_type {
            CompressionType::Bz2 => DBCompressionType::Bz2,
            CompressionType::Lz4 => DBCompressionType::Lz4,
            CompressionType::Lz4hc => DBCompressionType::Lz4hc,
            CompressionType::Snappy => DBCompressionType::Snappy,
            CompressionType::Zlib => DBCompressionType::Zlib,
            CompressionType::Zstd => DBCompressionType::Zstd,
            CompressionType::None => DBCompressionType::None,
        }
    }
}

impl Default for DbOptions {
    fn default() -> Self {
        Self::new(None, true, CompressionType::None)
    }
}
