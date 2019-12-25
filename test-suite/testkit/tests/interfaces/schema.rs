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
    merkledb::{
        access::{Access, FromAccess},
        MapIndex,
    },
};
use exonum_derive::*;
use exonum_proto::ProtobufConvert;
use serde_derive::{Deserialize, Serialize};

use crate::proto;

#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Wallet")]
pub struct Wallet {
    pub name: String,
    pub balance: u64,
}

#[derive(FromAccess)]
pub struct WalletSchema<T: Access> {
    pub wallets: MapIndex<T::Base, PublicKey, Wallet>,
}

impl<T: Access> WalletSchema<T> {
    pub fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}
