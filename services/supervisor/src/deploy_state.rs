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
    runtime::{execution_error_serde, ExecutionError},
};
use exonum_derive::*;
use exonum_proto::ProtobufConvert;
use failure::{self, format_err};
use serde_derive::{Deserialize, Serialize};

use crate::proto as pb_supervisor;

/// State of the deployment performed by `Supervisor`.
#[derive(Debug, Clone)]
#[derive(Serialize, Deserialize)]
#[derive(BinaryValue, ObjectHash)]
#[serde(rename_all = "snake_case")]
pub enum DeployState {
    /// Deployment is in process.
    Pending,
    /// Deadline reached.
    Timeout,
    /// Deployment resulted in a failure on a certain height.
    Failed {
        /// Height on which error happened.
        height: Height,
        /// Occurred error.
        #[serde(with = "execution_error_serde")]
        error: ExecutionError,
    },
    /// Deployment finished successfully.
    Succeed,
}

impl DeployState {
    /// Returns `true` if state of this deployment considered failed.
    pub fn is_failed(&self) -> bool {
        match self {
            DeployState::Timeout | DeployState::Failed { .. } => true,
            _ => false,
        }
    }

    /// Attempts to get a height from the state.
    /// Returns `None` if state is not `Failed`.
    pub fn height(&self) -> Option<Height> {
        match self {
            DeployState::Failed { height, .. } => Some(*height),
            _ => None,
        }
    }

    /// Attempts to get an execution error from the state.
    /// Returns `None` if state is not `Failed`.
    pub fn execution_error(&self) -> Option<ExecutionError> {
        match self {
            DeployState::Failed { error, .. } => Some(error.clone()),
            _ => None,
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
            DeployState::Timeout => pb.set_state(TIMEOUT),
            DeployState::Failed { height, error } => {
                let mut pb_error = pb_supervisor::ErrorInfo::new();

                pb_error.set_error(ProtobufConvert::to_pb(error));
                pb_error.set_height(height.0);

                pb.set_error(pb_error);
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
            TIMEOUT => DeployState::Timeout,
            FAIL => {
                if !pb.has_error() {
                    let error = format_err!(
                        "Protobuf representation of `DeployState` has type \
                         `FAIL` but has no cause set"
                    );
                    return Err(error);
                }
                let mut pb_error = pb.take_error();
                let error = ExecutionError::from_pb(pb_error.take_error())?;
                let height = Height(pb_error.get_height());
                DeployState::Failed { height, error }
            }
        };

        Ok(state)
    }
}
