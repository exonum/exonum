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
    merkledb::{Access, Entry, ObjectHash, RawAccessMut},
};

use super::ConfigPropose;

const NOT_INITIALIZED: &str = "Supervisor schema is not initialized";

pub struct Schema<T: Access> {
    pub config_propose: Entry<T::Base, ConfigPropose>,
}

impl<T: Access> Schema<T> {
    pub fn new(access: T) -> Self {
        Self {
            config_propose: access.entry("config_propose").expect(NOT_INITIALIZED),
        }
    }

    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.config_propose.object_hash()]
    }
}

impl<T> Schema<T>
where
    T: Access,
    T::Base: RawAccessMut,
{
    pub(super) fn initialize(access: T) -> Self {
        Self {
            config_propose: access.ensure_entry("config_propose"),
        }
    }
}
