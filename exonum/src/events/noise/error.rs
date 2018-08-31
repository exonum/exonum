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

use snow::SnowError;
#[derive(Fail, Debug)]
pub enum NoiseError {
    #[fail(display = "Wrong handshake message length {}", _0)]
    WrongMessageLength(usize),

    #[fail(display = "{}", _0)]
    Other(String),

    #[fail(display = "Snow error: {}", _0)]
    Snow(SnowError),
}

impl From<SnowError> for NoiseError {
    fn from(err: SnowError) -> NoiseError {
        NoiseError::Snow(err)
    }
}
