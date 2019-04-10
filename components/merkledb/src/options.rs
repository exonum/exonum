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

//! Abstract settings for databases.

use serde_derive::{Deserialize, Serialize};

/// Options for the database.
///
/// These parameters apply to the underlying database of Exonum, currently `RocksDB`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
}

impl Default for DbOptions {
    fn default() -> Self {
        Self {
            max_open_files: None,
            create_if_missing: true,
        }
    }
}
