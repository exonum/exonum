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

use exonum::{
    helpers::Height,
    runtime::{error::execution_error, ExecutionError},
};
use exonum_derive::*;
use exonum_proto::ProtobufConvert;
use failure::{self, format_err};
use serde_derive::{Deserialize, Serialize};

use crate::proto as pb_supervisor;

/// Reason for deployment failure.
#[derive(Debug, Clone)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[serde(rename_all = "snake_case")]
pub enum DeployFailCause {
    /// Deadline height was achieved with not enough confirmations.
    Deadline,
    /// At least one node failed the deployment attempt.
    /// Stored field contains the error from the *first* received transaction
    /// reporting a deployment failure. If two different nodes failed the deployment
    /// due to different reasons, it won't be represented in the cause.    
    DeployError {
        /// Height on which error happened.
        height: Height,
        /// Occurred error.
        #[serde(with = "execution_error")]
        error: ExecutionError,
    },
}

impl DeployFailCause {
    /// Attempts to get a height from the fail cause.
    /// Returns `None` if cause is deadline.
    pub fn height(&self) -> Option<Height> {
        match self {
            DeployFailCause::Deadline => None,
            DeployFailCause::DeployError { height, .. } => Some(*height),
        }
    }

    /// Attempts to get an execution error from the fail cause.
    /// Returns `None` if cause is deadline.
    pub fn execution_error(&self) -> Option<ExecutionError> {
        match self {
            DeployFailCause::Deadline => None,
            DeployFailCause::DeployError { error, .. } => Some(error.clone()),
        }
    }
}

/// State of the deployment performed by `Supervisor`.
#[derive(Debug, Clone)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[serde(rename_all = "snake_case")]
pub enum DeployState {
    /// Deployment is in process.
    Pending,
    /// Deployment resulted in a failure on a certain height.
    Failed(DeployFailCause),
    /// Deployment finished successfully.
    Succeed,
}

impl ProtobufConvert for DeployFailCause {
    type ProtoStruct = pb_supervisor::DeployFailCause;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut pb = Self::ProtoStruct::new();
        match self {
            DeployFailCause::Deadline => pb.set_deadline(Default::default()),
            DeployFailCause::DeployError { height, error } => {
                let mut error_info_pb = pb_supervisor::ErrorInfo::new();
                let pb_error = ProtobufConvert::to_pb(error);
                error_info_pb.set_error(pb_error);
                error_info_pb.set_height(height.0);
                pb.set_error(error_info_pb)
            }
        }
        pb
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        if pb.has_deadline() {
            Ok(DeployFailCause::Deadline)
        } else if pb.has_error() {
            let mut error_info_pb = pb.take_error();
            let error = ExecutionError::from_pb(error_info_pb.take_error())?;
            let height = Height(error_info_pb.get_height());
            let reason = DeployFailCause::DeployError { height, error };
            Ok(reason)
        } else {
            Err(format_err!("Invalid `DeployFailCause` format"))
        }
    }
}

impl ProtobufConvert for DeployState {
    type ProtoStruct = pb_supervisor::DeployState;

    fn to_pb(&self) -> Self::ProtoStruct {
        use pb_supervisor::DeployState_Type::*;

        let mut pb = Self::ProtoStruct::new();
        match self {
            DeployState::Pending => pb.set_state(PENDING),
            DeployState::Succeed => pb.set_state(SUCCESS),
            DeployState::Failed(cause) => {
                let pb_cause = ProtobufConvert::to_pb(cause);
                pb.set_cause(pb_cause);
                pb.set_state(FAIL);
            }
        }
        pb
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        use pb_supervisor::DeployState_Type::*;
        let state = match pb.get_state() {
            PENDING => DeployState::Pending,
            SUCCESS => DeployState::Succeed,
            FAIL => {
                if !pb.has_cause() {
                    return Err(format_err!("Protobuf representation of `DeployState` has type `FAIL` but has no cause set"));
                }
                let cause = DeployFailCause::from_pb(pb.take_cause())?;
                DeployState::Failed(cause)
            }
        };

        Ok(state)
    }
}
