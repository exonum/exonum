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
    #[serde(with = "execution_error")]
    DeployError(ExecutionError),
}

/// State of the deployment performed by `Supervisor`.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[serde(rename_all = "snake_case")]
pub enum DeployState {
    /// Deploy with provided spec was not requested.
    NotRequested,
    /// Deployment is in process.
    Pending,
    /// Deployment resulted in a failure on a certain height.
    Failed(Height, DeployFailCause),
    /// Deployment finished successfully.
    Succeed,
}

impl ProtobufConvert for DeployFailCause {
    type ProtoStruct = pb_supervisor::DeployFailCause;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut pb = Self::ProtoStruct::new();
        match self {
            DeployFailCause::Deadline => pb.set_deadline(Default::default()),
            DeployFailCause::DeployError(error) => {
                let pb_error = ProtobufConvert::to_pb(error);
                pb.set_error(pb_error)
            }
        }
        pb
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        if pb.has_deadline() {
            Ok(DeployFailCause::Deadline)
        } else if pb.has_error() {
            let error = ExecutionError::from_pb(pb.take_error())?;
            let reason = DeployFailCause::DeployError(error);
            Ok(reason)
        } else {
            Err(format_err!("Invalid `DeployFailCause` format"))
        }
    }
}

impl PartialEq for DeployFailCause {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (DeployFailCause::Deadline, DeployFailCause::Deadline) => true,
            (
                DeployFailCause::DeployError(error_self),
                DeployFailCause::DeployError(error_other),
            ) => error_self.to_match() == *error_other,
            _ => false,
        }
    }
}

impl ProtobufConvert for DeployState {
    type ProtoStruct = pb_supervisor::DeployState;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut pb = Self::ProtoStruct::new();
        match self {
            DeployState::NotRequested => pb.set_not_requested(Default::default()),
            DeployState::Pending => pb.set_pending(Default::default()),
            DeployState::Succeed => pb.set_succeed(Default::default()),
            DeployState::Failed(height, cause) => {
                let pb_cause = ProtobufConvert::to_pb(cause);
                let mut pb_failure_info = pb_supervisor::FailureInfo::new();
                pb_failure_info.set_height(height.0);
                pb_failure_info.set_cause(pb_cause);
                pb.set_failed(pb_failure_info);
            }
        }
        pb
    }

    fn from_pb(mut pb: Self::ProtoStruct) -> Result<Self, failure::Error> {
        if pb.has_not_requested() {
            Ok(DeployState::NotRequested)
        } else if pb.has_pending() {
            Ok(DeployState::Pending)
        } else if pb.has_succeed() {
            Ok(DeployState::Succeed)
        } else if pb.has_failed() {
            let mut pb_failure_info = pb.take_failed();
            let cause = DeployFailCause::from_pb(pb_failure_info.take_cause())?;
            let height = pb_failure_info.get_height();
            let reason = DeployState::Failed(Height(height), cause);
            Ok(reason)
        } else {
            Err(format_err!("Invalid `DeployState` format"))
        }
    }
}
