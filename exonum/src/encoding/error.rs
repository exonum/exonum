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

use std::{borrow::Cow, error::Error as StdError, fmt};

#[derive(Debug)]
/// This structure represent `encoding` specific errors.
/// This errors returned by function `check` of each `Field`.
pub enum Error {
    /// Overflow in Offsets.
    OffsetOverflow,
    /// Overflow in Duration.
    DurationOverflow,
    /// Incorrect duration representation.
    IncorrectDuration {
        /// Seconds in gotten duration.
        secs: i64,
        /// Nanoseconds in gotten duration.
        nanos: i32,
    },
    /// Basic error support, for custom fields.
    Basic(Cow<'static, str>),
    /// Other error for custom fields.
    Other(Box<dyn StdError + Send + Sync + 'static>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} = {:?}", self.description(), self)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::OffsetOverflow => "Offset pointers overflow",
            Error::DurationOverflow => "Overflow in Duration object",
            Error::IncorrectDuration { .. } => "Incorrect Duration object representation",
            Error::Basic(_) | Error::Other(_) => "Other error",
        }
    }

    fn cause(&self) -> Option<&dyn StdError> {
        use std::ops::Deref;
        if let Error::Other(ref error) = *self {
            Some(error.deref())
        } else {
            None
        }
    }
}

impl From<Box<dyn StdError + Send + Sync + 'static>> for Error {
    fn from(t: Box<dyn StdError + Send + Sync + 'static>) -> Self {
        Error::Other(t)
    }
}

impl From<Cow<'static, str>> for Error {
    fn from(t: Cow<'static, str>) -> Self {
        Error::Basic(t)
    }
}

impl From<&'static str> for Error {
    fn from(t: &'static str) -> Self {
        Error::Basic(t.into())
    }
}
