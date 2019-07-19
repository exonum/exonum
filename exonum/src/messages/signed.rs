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

use protobuf::Message;

use std::convert::TryFrom;

use crate::{
    crypto::{self, Signature},
    proto::ProtobufConvert,
};

use super::types::{Connect, Signed};

#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
pub struct Verified<T: ProtobufConvert> {
    raw: Signed,
    payload: T,
}

// impl<T> TryFrom<Signed> for Verified<T>
// where
//     T: ProtobufConvert,
//     T::ProtoStruct: Message + Default,
// {
//     type Error = failure::Error;

//     fn try_from(raw: Signed) -> Result<Self, Self::Error> {
//         // Verifies message signature
//         ensure!(
//             crypto::verify(&raw.signature, &raw.payload, &raw.author),
//             "Failed to verify signature."
//         );
//         // Deserializes message.
//         let mut inner = <T::ProtoStruct>::default();
//         inner.merge_from_bytes(&raw.payload)?;
//         let payload = T::from_pb(inner)?;
//         Ok(Self { raw, payload })
//     }
// }

// impl TryFrom<Protocol> for Connect {
//     type Error = failure::Error;

//     fn try_from(value: Protocol) -> Result<Self, Self::Error> {
//         match value.message {
//             Some(Protocol_oneof_message::connect(inner)) => Self::from_pb(inner),
//             _ => Err(format_err!("Unknown message"))
//         }
//     }
// }
