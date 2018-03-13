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

//! Abstract settings for databases.

/// Options for database.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DbOptions {
    /// Number of open files that can be used by the DB.
    ///
    /// Defaults to `None`, which means opened files are always kept open.
    pub max_open_files: Option<i32>,
    /// Whether create database or not, if it's missing.
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
