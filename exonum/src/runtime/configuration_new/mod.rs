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

// use semver::Version;

use crate::{
    // crypto::{Hash, PublicKey},
    messages::BinaryForm,
    runtime::{
        error::{ExecutionError, WRONG_ARG_ERROR},
        // CallInfo, DeployStatus, InstanceInitData, RuntimeContext, RuntimeEnvironment,
        // ServiceInstanceId,
        rust::{service::Service, TransactionContext},
    },
    // storage::{Database, Entry, MemoryDB},
    proto::schema::configuration::ConfigurationServiceInit,
};
// use protobuf::{well_known_types::Any, Message};
use protobuf::{well_known_types::Any};

mod transactions;
mod config;
mod errors;
mod schema;

/// Service identifier for the configuration service.
pub const SERVICE_ID: u16 = 1;
/// Configuration service name.
pub const SERVICE_NAME: &str = "configuration";

service_interface! {
    trait ConfigurationService {
        fn method_a(&self, ctx: TransactionContext, arg: transactions::Propose) -> Result<(), ExecutionError>;
        fn method_b(&self, ctx: TransactionContext, arg: transactions::Vote) -> Result<(), ExecutionError>;
    }
}

#[derive(Debug, Default)]
pub struct ConfigurationServiceImpl {
    pub majority_count: Option<u32>,
}

impl ConfigurationService for ConfigurationServiceImpl {
    fn method_a(&self, mut _ctx: TransactionContext, _arg: transactions::Propose) -> Result<(), ExecutionError> {
        Ok(())
    }

    fn method_b(&self, mut _ctx: TransactionContext, _arg: transactions::Vote) -> Result<(), ExecutionError> {
        Ok(())
    }
}

impl_service_dispatcher!(ConfigurationServiceImpl, ConfigurationService);
impl Service for ConfigurationServiceImpl {
    fn initialize(&mut self, mut _ctx: TransactionContext, arg: Any) -> Result<(), ExecutionError> {
        let arg: ConfigurationServiceInit = BinaryForm::decode(arg.get_value()).map_err(|e| {
            ExecutionError::with_description(WRONG_ARG_ERROR, format!("Wrong argument: {}", e))
        })?;

        if arg.is_custom_majority_count {
            self.majority_count = Some(arg.majority_count)
        }

        Ok(())
    }
}