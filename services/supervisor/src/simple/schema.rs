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

use exonum::merkledb::{
    access::{Access, FromAccess, Prefixed},
    ProofEntry,
};

use super::ConfigPropose;

pub struct Schema<T: Access> {
    pub config_propose: ProofEntry<T::Base, ConfigPropose>,
}

impl<'a, T: Access> Schema<Prefixed<'a, T>> {
    pub fn new(access: Prefixed<'a, T>) -> Self {
        Self {
            config_propose: FromAccess::from_access(access, "config_propose".into()).unwrap(),
        }
    }
}
