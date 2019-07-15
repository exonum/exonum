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

//! The set of errors for the Runtime module.

use exonum_merkledb::{BinaryValue, ObjectHash};
use protobuf::Message;

use std::convert::TryFrom;

use crate::{
    crypto::{self, Hash},
    proto::{schema::runtime, ProtobufConvert},
};

/// Kind of execution error, indicates in which module error occurred.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ErrorKind {
    // Operation execution has been finished with panic.
    Panic,
    // An error in dispatcher during the execution occurred.
    Dispatcher {
        /// Error code.
        code: u8,
    },
    // An error in the runtime occurred.
    Runtime {
        /// User-defined error code.
        /// Error codes can have different meanings for the different runtimes.
        code: u8,
    },
    // An error during the service's transaction execution occurred.
    Service {
        /// User-defined error code.
        /// Error codes can have different meanings for the different transactions 
        /// and services.        
        code: u8,
    },
}

impl ErrorKind {
    /// Creates dispatcher error with the specified code.
    pub fn dispatcher(code: impl Into<u8>) -> Self {
        ErrorKind::Dispatcher { code: code.into() }
    }

    /// Creates runtime error with the specified code.
    pub fn runtime(code: impl Into<u8>) -> Self {
        ErrorKind::Runtime { code: code.into() }
    }

    /// Creates service error with the specified code.
    pub fn service(code: impl Into<u8>) -> Self {
        ErrorKind::Service { code: code.into() }
    }

    fn into_raw(self) -> (u8, u8) {
        match self {
            ErrorKind::Panic => (0, 0),
            ErrorKind::Dispatcher { code } => (1, code),
            ErrorKind::Runtime { code } => (2, code),
            ErrorKind::Service { code } => (3, code),
        }
    }

    fn from_raw(kind: u8, code: u8) -> Result<Self, failure::Error> {
        match kind {
            0 => {
                ensure!(code != 0, "Error code for panic should be zero");
                Ok(ErrorKind::Panic)
            }
            1 => Ok(ErrorKind::Dispatcher { code }),
            2 => Ok(ErrorKind::Runtime { code }),
            3 => Ok(ErrorKind::Service { code }),
            _ => bail!("Unknown error kind"),
        }
    }
}

/// Result of unsuccessful runtime execution.
///
/// An execution error consists of an error kind and optional description.
/// The error code affects the blockchain state hash, while the description does not.
/// Therefore descriptions are mostly used for developer purposes, not for interaction of
/// the system with users.
///
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionError {
    /// The kind of error that indicates in which module and with which code the error occurred.
    pub kind: ErrorKind,
    /// Optional description which doesn't affect `object_hash`.
    pub description: String,
}

impl ExecutionError {
    /// Creates a new execution error instance with the specified kind and optional description.
    pub fn new(kind: ErrorKind, description: impl Into<String>) -> Self {
        Self {
            kind,
            description: description.into(),
        }
    }
}

impl ProtobufConvert for ExecutionError {
    type ProtoStruct = runtime::ExecutionError;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut inner = Self::ProtoStruct::default();
        let (kind, code) = self.kind.into_raw();
        inner.set_kind(u32::from(kind));
        inner.set_code(u32::from(code));
        inner
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let kind = u8::try_from(pb.get_kind())?;
        let code = u8::try_from(pb.get_code())?;
        Ok(Self {
            kind: ErrorKind::from_raw(kind, code)?,
            description: pb.take_description(),
        })
    }
}

impl BinaryValue for ExecutionError {
    fn to_bytes(&self) -> Vec<u8> {
        self.to_pb()
            .write_to_bytes()
            .expect("Failed to serialize in BinaryValue for ExecutionError")
    }

    fn from_bytes(value: std::borrow::Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut inner = <Self as ProtobufConvert>::ProtoStruct::new();
        inner.merge_from_bytes(value.as_ref())?;
        ProtobufConvert::from_pb(inner)
    }
}

impl ObjectHash for ExecutionError {
    fn object_hash(&self) -> Hash {
        let (kind, code) = self.kind.into_raw();
        crypto::hash(&[kind, code])
    }
}

#[test]
fn execution_error_binary_value_round_trip() {
    let values = vec![
        (ErrorKind::Panic, "AAAA"),
        (ErrorKind::Dispatcher { code: 0 }, ""),
        (ErrorKind::Dispatcher { code: 0 }, "b"),
        (ErrorKind::Runtime { code: 1 }, "c"),
        (ErrorKind::Service { code: 18 }, "ddc"),
    ];

    for (kind, description) in values {
        let err = ExecutionError {
            kind,
            description: description.to_owned(),
        };

        let bytes = err.to_bytes();
        let err2 = ExecutionError::from_bytes(bytes.into()).unwrap();
        assert_eq!(err, err2);
    }
}

#[test]
fn execution_error_binary_value_wrong_kind() {
    let bytes = {
        let mut inner = runtime::ExecutionError::default();
        inner.set_kind(117);
        inner.set_code(2);
        inner.write_to_bytes().unwrap()
    };

    assert_eq!(
        ExecutionError::from_bytes(bytes.into())
            .unwrap_err()
            .to_string(),
        "Unknown error kind"
    )
}

#[test]
fn execution_error_object_hash_description() {
    let first_err = ExecutionError {
        kind: ErrorKind::Service { code: 5 },
        description: "foo".to_owned(),
    };

    let second_err = ExecutionError {
        kind: ErrorKind::Service { code: 5 },
        description: "foo bar".to_owned(),
    };

    assert_eq!(first_err.object_hash(), second_err.object_hash());
}
