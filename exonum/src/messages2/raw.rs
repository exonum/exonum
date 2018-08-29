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
use super::{Message, ProtocolMessage};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UncheckedBuffer {
    message: Vec<u8>,
}

impl UncheckedBuffer {
    pub fn new(vec: Vec<u8>) -> UncheckedBuffer {
        UncheckedBuffer { message: vec }
    }
    pub fn get_vec(&self) -> &Vec<u8> {
        &self.message
    }
}

impl ::std::convert::AsRef<[u8]> for UncheckedBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.message
    }
}

impl<T: ProtocolMessage> From<Message<T>> for UncheckedBuffer {
    fn from(val: Message<T>) -> UncheckedBuffer {
        UncheckedBuffer::new(val.into_parts().1.to_vec())
    }
}
