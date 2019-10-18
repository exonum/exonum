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
    crypto::PublicKey,
    merkledb::{IndexAccess, MapIndex},
};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_proto_derive::ProtobufConvert;
use serde_derive::{Deserialize, Serialize};

use crate::proto;

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Wallet")]
pub struct Wallet {
    pub name: String,
    pub balance: u64,
}

pub struct WalletSchema<T>(T);

impl<T: IndexAccess> WalletSchema<T> {
    pub fn new(access: T) -> Self {
        Self(access)
    }

    pub fn wallets(&self) -> MapIndex<T, PublicKey, Wallet> {
        MapIndex::new("wallets", self.0.clone())
    }
}
