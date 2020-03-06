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
// limitations under the License.

use anyhow::ensure;
use exonum_derive::*;
use exonum_proto::ProtobufConvert;

use super::ExecutionError;
use crate::proto::schema::errors as errors_proto;

/// Status of a call execution in a way it is stored in the blockchain.
/// This result may be either an empty unit type, in case of success,
/// or an `ExecutionError`, if execution has failed.
#[derive(Clone, Debug, Serialize, Deserialize, BinaryValue)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ExecutionStatus(#[serde(with = "self::serde")] pub Result<(), ExecutionError>);

impl ExecutionStatus {
    /// Creates status for the successful execution.
    pub fn ok() -> Self {
        Self(Ok(()))
    }

    /// Creates status for the failed execution.
    pub fn err(err: impl Into<ExecutionError>) -> Self {
        Self(Err(err.into()))
    }
}

impl From<Result<(), ExecutionError>> for ExecutionStatus {
    fn from(inner: Result<(), ExecutionError>) -> Self {
        Self(inner)
    }
}

impl ProtobufConvert for ExecutionStatus {
    type ProtoStruct = errors_proto::ExecutionStatus;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut inner = Self::ProtoStruct::default();
        match &self.0 {
            Result::Ok(_) => inner.set_ok(protobuf::well_known_types::Empty::new()),
            Result::Err(e) => inner.set_error(e.to_pb()),
        }
        inner
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> anyhow::Result<Self> {
        let inner = if pb.has_error() {
            ensure!(!pb.has_ok(), "ExecutionStatus has both of variants.");
            Err(ExecutionError::from_pb(pb.take_error())?)
        } else {
            Ok(())
        };
        Ok(Self(inner))
    }
}

pub mod serde {
    //! More convenient `serde` layout for `Result<(), ExecutionError>`.

    use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

    use crate::runtime::{CallSite, ErrorKind, ExecutionError};

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum ExecutionType {
        Success,
        UnexpectedError,
        CommonError,
        CoreError,
        RuntimeError,
        ServiceError,
    }

    /// Version of `ExecutionStatus` suitable for `serde`.
    #[doc(hidden)]
    #[derive(Debug, Serialize, Deserialize)]
    pub struct ExecutionStatus {
        #[serde(rename = "type")]
        typ: ExecutionType,
        #[serde(skip_serializing_if = "String::is_empty", default)]
        description: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        code: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        runtime_id: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        call_site: Option<CallSite>,
    }

    impl From<Result<(), &ExecutionError>> for ExecutionStatus {
        fn from(inner: Result<(), &ExecutionError>) -> Self {
            if let Err(err) = inner {
                let (typ, code) = match err.kind {
                    ErrorKind::Unexpected => (ExecutionType::UnexpectedError, None),
                    ErrorKind::Common { code } => (ExecutionType::CommonError, Some(code)),
                    ErrorKind::Core { code } => (ExecutionType::CoreError, Some(code)),
                    ErrorKind::Runtime { code } => (ExecutionType::RuntimeError, Some(code)),
                    ErrorKind::Service { code } => (ExecutionType::ServiceError, Some(code)),
                };

                Self {
                    typ,
                    description: err.description.clone(),
                    code,
                    runtime_id: err.runtime_id,
                    call_site: err.call_site.clone(),
                }
            } else {
                Self {
                    typ: ExecutionType::Success,
                    description: String::new(),
                    code: None,
                    runtime_id: None,
                    call_site: None,
                }
            }
        }
    }

    impl ExecutionStatus {
        /// Converts an execution status from an untrusted format (e.g., received in JSON via HTTP API)
        /// into an actionable `Result`.
        pub(crate) fn into_result(self) -> Result<Result<(), ExecutionError>, &'static str> {
            Ok(if let ExecutionType::Success = self.typ {
                Ok(())
            } else {
                let kind = match self.typ {
                    ExecutionType::UnexpectedError => {
                        if self.code != None {
                            return Err("Code specified for an unexpected error");
                        }
                        ErrorKind::Unexpected
                    }
                    ExecutionType::CommonError => ErrorKind::Common {
                        code: self.code.ok_or("No code specified")?,
                    },
                    ExecutionType::CoreError => ErrorKind::Core {
                        code: self.code.ok_or("No code specified")?,
                    },
                    ExecutionType::RuntimeError => ErrorKind::Runtime {
                        code: self.code.ok_or("No code specified")?,
                    },
                    ExecutionType::ServiceError => ErrorKind::Service {
                        code: self.code.ok_or("No code specified")?,
                    },
                    ExecutionType::Success => unreachable!(),
                };

                Err(ExecutionError {
                    kind,
                    description: self.description,
                    runtime_id: self.runtime_id,
                    call_site: self.call_site,
                })
            })
        }
    }

    pub fn serialize<S>(
        inner: &Result<(), ExecutionError>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ExecutionStatus::from(inner.as_ref().map(|_| ())).serialize(serializer)
    }

    pub fn deserialize<'a, D>(deserializer: D) -> Result<Result<(), ExecutionError>, D::Error>
    where
        D: Deserializer<'a>,
    {
        ExecutionStatus::deserialize(deserializer)
            .and_then(|status| status.into_result().map_err(D::Error::custom))
    }
}
