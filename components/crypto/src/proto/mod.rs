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

pub use crate::proto::schema::*;

use exonum_proto::ProtobufConvert;
use failure::{ensure, format_err, Error};

use crate::{Hash, PublicKey, Signature};
use crate::{HASH_SIZE, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};

mod schema;
#[cfg(test)]
mod tests;

impl ProtobufConvert for Hash {
    type ProtoStruct = schema::Hash;

    fn to_pb(&self) -> schema::Hash {
        let mut hash = schema::Hash::new();
        hash.set_data(self.as_ref().to_vec());
        hash
    }

    fn from_pb(pb: schema::Hash) -> Result<Self, Error> {
        let data = pb.get_data();
        ensure!(data.len() == HASH_SIZE, "Wrong Hash size");
        crate::Hash::from_slice(data).ok_or_else(|| format_err!("Cannot convert Hash from bytes"))
    }
}

impl ProtobufConvert for PublicKey {
    type ProtoStruct = schema::PublicKey;

    fn to_pb(&self) -> schema::PublicKey {
        let mut key = schema::PublicKey::new();
        key.set_data(self.as_ref().to_vec());
        key
    }

    fn from_pb(pb: schema::PublicKey) -> Result<Self, Error> {
        let data = pb.get_data();
        ensure!(data.len() == PUBLIC_KEY_LENGTH, "Wrong PublicKey size");
        crate::PublicKey::from_slice(data)
            .ok_or_else(|| format_err!("Cannot convert PublicKey from bytes"))
    }
}

impl ProtobufConvert for Signature {
    type ProtoStruct = schema::Signature;

    fn to_pb(&self) -> schema::Signature {
        let mut sign = schema::Signature::new();
        sign.set_data(self.as_ref().to_vec());
        sign
    }

    fn from_pb(pb: schema::Signature) -> Result<Self, Error> {
        let data = pb.get_data();
        ensure!(data.len() == SIGNATURE_LENGTH, "Wrong Signature size");
        crate::Signature::from_slice(data)
            .ok_or_else(|| format_err!("Cannot convert Signature from bytes"))
    }
}
