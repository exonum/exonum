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

use exonum_proto::ProtobufConvert;
use crate::proto;

#[test]
fn test_bitvec_pb_convert() {

}

mod dt {
    use crate::proto;
    use exonum_derive::{BinaryValue, ObjectHash};
    use exonum_proto_derive::ProtobufConvert;
    use exonum_merkledb::BinaryValue;
    use exonum_proto::ProtobufConvert;
    use protobuf::Message;

    #[derive(ProtobufConvert, BinaryValue, ObjectHash)]
    #[exonum(pb = "proto::common::SimpleMessage")]
    pub struct SimpleMessage {
        pub len: u64,
        pub data: Vec<u8>,
    }
}

use exonum_merkledb::{ObjectHash, BinaryValue};

#[test]
fn test_derive() {
    let dt = dt::SimpleMessage { len: 12, data: Vec::new() };

    dbg!(dt.to_bytes());
    dbg!(dt.object_hash());
}
