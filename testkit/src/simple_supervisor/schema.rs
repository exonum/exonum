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

use exonum::{
    crypto::Hash,
    merkledb::{Entry, IndexAccess, ObjectHash},
};

use super::ConfigPropose;

pub struct Schema<T: IndexAccess> {
    access: T,
}

impl<T: IndexAccess> Schema<T> {
    pub fn new(access: T) -> Self {
        Self { access }
    }

    pub fn config_propose_entry(&self) -> Entry<T, ConfigPropose> {
        Entry::new("config.propose", self.access.clone())
    }

    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.config_propose_entry().object_hash()]
    }
}
