// Copyright 2017 The Exonum Team
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

use crypto::SecretKey;
use messages::raw::{FromRaw, MessageBuffer};
use messages::RawMessage;

#[test]
fn test_message_without_fields() {
    message! {
        struct NoFields {
            const TYPE = 0;
            const ID = 0;
            const SIZE = 0;
        }
    }
    drop(NoFields::new(&SecretKey::new([1; 64])));
}

#[test]
#[allow(dead_code)]
#[should_panic(expected = "Found error in from_raw: UnexpectedlyShortPayload")]
fn test_message_with_small_size() {
    message! {
        struct SmallField {
            const TYPE = 0;
            const ID = 0;
            const SIZE = 1;
            field test: bool [0 => 1]
        }
    }

    let buff = vec![1; 1];
    let raw = RawMessage::new(MessageBuffer::from_vec(buff));
    let _message = <SmallField as FromRaw>::from_raw(raw).expect("Found error in from_raw");
}
