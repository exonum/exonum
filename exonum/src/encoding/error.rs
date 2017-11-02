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

use std::error::Error as StdError;
use std::fmt;
use std::borrow::Cow;

use super::Offset;

#[derive(Debug)]
/// This structure represent `encoding` specific errors.
/// This errors returned by function `check` of each `Field`.
pub enum Error {
    // TODO: Check this message after refactor buffer (ECR-156).
    /// Payload is short for this message.
    UnexpectedlyShortPayload {
        /// real message size
        actual_size: Offset,
        /// expected size of fixed part
        minimum_size: Offset,
    },
    /// Boolean value is incorrect
    IncorrectBoolean {
        /// position in buffer where error apears
        position: Offset,
        /// value that was parsed as bool
        value: u8,
    },
    /// Segment reference is incorrect
    IncorrectSegmentReference {
        /// position in buffer where error apears
        position: Offset,
        /// value that was parsed as segment reference
        value: Offset,
    },
    /// Segment size is incorrect
    IncorrectSegmentSize {
        /// position in buffer where error apears
        position: Offset,
        /// value that was parsed as size
        value: Offset,
    },
    /// `RawMessage` is to short
    UnexpectedlyShortRawMessage {
        /// position in buffer where error apears
        position: Offset,
        /// size of raw message in buffer
        size: Offset,
    },
    /// Incorrect size of `RawMessage` found in buffer
    IncorrectSizeOfRawMessage {
        /// position in buffer where error apears
        position: Offset,
        /// parsed message size
        actual_size: Offset,
        /// expected fixed part message size
        declared_size: Offset,
    },
    /// Incorrect `message_id` found in buffer.
    IncorrectMessageType {
        /// expected `message_id`
        message_type: u16,
    },
    /// Different segments overlaps
    OverlappingSegment {
        /// last segment ended position
        last_end: Offset,
        /// start of new segment
        start: Offset,
    },
    /// Between segments foud spaces
    SpaceBetweenSegments {
        /// last segment ended position
        last_end: Offset,
        /// start of new segment
        start: Offset,
    },
    /// Error in parsing `Utf8` `String`
    Utf8 {
        /// position in buffer where error apears
        position: Offset,
        /// what error exact was
        error: ::std::str::Utf8Error,
    },
    /// Overflow in Offsets
    OffsetOverflow,
    /// Basic error suport, for custom fields
    Basic(Cow<'static, str>),
    /// Other error for custom fields
    Other(Box<StdError>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} = {:?}", self.description(), self)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::UnexpectedlyShortPayload { .. } => "Unexpectedly short payload",
            Error::IncorrectBoolean { .. } => "Incorrect boolean value",
            Error::IncorrectSegmentReference { .. } => "Incorrect segment reference",
            Error::IncorrectSegmentSize { .. } => "Incorrect segment size",
            Error::UnexpectedlyShortRawMessage { .. } => "Unexpectedly short RawMessage",
            Error::IncorrectSizeOfRawMessage { .. } => "Incorrect size of RawMessage",
            Error::IncorrectMessageType { .. } => "Incorrect message type",
            Error::OverlappingSegment { .. } => "Overlapping segments",
            Error::SpaceBetweenSegments { .. } => "Space between segments",
            Error::Utf8 { .. } => "Utf8 error in parsing string",
            Error::OffsetOverflow => "Offset pointers overflow",
            Error::Basic(ref x) => x.as_ref(),
            Error::Other(_) => "Other error",
        }
    }

    fn cause(&self) -> Option<&StdError> {
        use std::ops::Deref;
        if let Error::Other(ref error) = *self {
            Some(error.deref())
        } else {
            None
        }
    }
}

impl From<Box<StdError>> for Error {
    fn from(t: Box<StdError>) -> Error {
        Error::Other(t)
    }
}

impl From<Cow<'static, str>> for Error {
    fn from(t: Cow<'static, str>) -> Error {
        Error::Basic(t)
    }
}

impl From<&'static str> for Error {
    fn from(t: &'static str) -> Error {
        Error::Basic(t.into())
    }
}
