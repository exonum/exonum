// Copyright 2020 The Exonum Team
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

/// Implement `BinaryValue` trait for type that implements `Protobuf::Message`.
#[macro_export]
macro_rules! impl_binary_value_for_pb_message {
    ($( $type:ty ),*) => {
        $(
            impl BinaryValue for $type {
                fn to_bytes(&self) -> Vec<u8> {
                    use protobuf::Message;
                    self.write_to_bytes().expect("Error while serializing value")
                }

                fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Result<Self, failure::Error> {
                    use protobuf::Message;
                    let mut pb = Self::new();
                    pb.merge_from_bytes(bytes.as_ref())?;
                    Ok(pb)
                }
            }
        )*
    };
}
