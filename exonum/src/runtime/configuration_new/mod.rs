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

pub use self::transactions::{Deploy, Init};

use exonum_merkledb::{BinaryValue, Fork, IndexAccess, Snapshot};
use protobuf::well_known_types::Any;

use crate::{
    blockchain::Schema as CoreSchema,
    crypto::Hash,
    node::State,
    runtime::{
        dispatcher::Dispatcher,
        error::{ExecutionError, InitError, WRONG_ARG_ERROR},
        rust::{
            service::{Service, ServiceFactory},
            RustArtifactSpec, TransactionContext,
        },
        DeployStatus, RuntimeEnvironment, ServiceConstructor,
    },
};

use crate::{
    messages::{MethodId, ServiceInstanceId},
    runtime::rust::RustRuntime,
};

use self::{
    config::ConfigurationServiceConfig,
    errors::Error as ServiceError,
    schema::{Schema as ConfigurationSchema, VotingDecision},
    transactions::{enough_votes_to_commit, VotingContext},
};

mod config;
mod errors;
mod schema;
mod transactions;

/// Service identifier for the configuration service.
pub const SERVICE_ID: ServiceInstanceId = 0;
pub const DEPLOY_METHOD_ID: MethodId = 3;
pub const INIT_METHOD_ID: MethodId = 4;
/// Configuration service name.
pub const SERVICE_NAME: &str = "configuration";

/// Constant artifact spec.
pub fn artifact_spec() -> RustArtifactSpec {
    RustArtifactSpec {
        name: "core.config".to_owned(),
        version: semver::Version::new(0, 1, 0),
    }
}

#[service_interface(exonum(crate = "crate"))]
trait ConfigurationService {
    fn propose(
        &self,
        ctx: TransactionContext,
        tx: transactions::Propose,
    ) -> Result<(), ExecutionError>;

    fn vote(&self, ctx: TransactionContext, arg: transactions::Vote) -> Result<(), ExecutionError>;

    fn vote_against(
        &self,
        ctx: TransactionContext,
        arg: transactions::VoteAgainst,
    ) -> Result<(), ExecutionError>;

    fn deploy(
        &self,
        ctx: TransactionContext,
        arg: transactions::Deploy,
    ) -> Result<(), ExecutionError>;

    fn init(&self, ctx: TransactionContext, arg: transactions::Init) -> Result<(), ExecutionError>;
}

#[derive(Debug)]
pub struct ConfigurationServiceImpl {
    dispatcher: *mut Dispatcher,
}

impl ConfigurationServiceImpl {
    fn assign_service_id(&self, fork: &Fork, instance_name: &String) -> Option<u32> {
        let schema = ConfigurationSchema::new(fork);
        let mut service_ids = schema.service_ids();

        if service_ids.contains(instance_name) {
            return None;
        }

        let id = service_ids.iter().count() as u32 + 1; // TODO O(n) optimize
        service_ids.put(instance_name, id);

        Some(id)
    }

    pub fn get_id_for(&self, snapshot: &dyn Snapshot, instance_name: &String) -> Option<u32> {
        let schema = ConfigurationSchema::new(snapshot);
        let service_ids = schema.service_ids();

        service_ids.get(instance_name)
    }

    #[allow(unsafe_code)]
    fn get_dispatcher_mut(&self) -> &mut Dispatcher {
        unsafe { self.dispatcher.as_mut().unwrap() }
    }
}

impl ConfigurationService for ConfigurationServiceImpl {
    fn propose(
        &self,
        ctx: TransactionContext,
        tx: transactions::Propose,
    ) -> Result<(), ExecutionError> {
        let author = ctx.author();
        let fork = ctx.fork();
        let (cfg, cfg_hash) = tx.precheck(fork.snapshot(), author).map_err(|err| {
            error!("Discarding propose {:?}: {}", self, err);
            err
        })?;

        tx.save(fork, &cfg, cfg_hash);
        trace!("Put propose {:?} to config_proposes table", self);
        Ok(())
    }

    fn vote(&self, ctx: TransactionContext, tx: transactions::Vote) -> Result<(), ExecutionError> {
        let author = ctx.author();
        let tx_hash = ctx.tx_hash();
        let fork = ctx.fork();
        let decision = VotingDecision::Yea(tx_hash);

        let vote = VotingContext::new(decision, author, tx.cfg_hash);
        let parsed_config = vote.precheck(fork.snapshot()).map_err(|err| {
            error!("Discarding vote {:?}: {}", tx, err);
            err
        })?;

        vote.save(fork);
        trace!(
            "Put Vote:{:?} to corresponding cfg votes_by_config_hash table",
            tx
        );

        if enough_votes_to_commit(fork.snapshot(), &tx.cfg_hash) {
            CoreSchema::new(fork).commit_configuration(parsed_config);
        }
        Ok(())
    }

    fn vote_against(
        &self,
        ctx: TransactionContext,
        tx: transactions::VoteAgainst,
    ) -> Result<(), ExecutionError> {
        let author = ctx.author();
        let tx_hash = ctx.tx_hash();
        let fork = ctx.fork();
        let decision = VotingDecision::Nay(tx_hash);

        let vote_against = VotingContext::new(decision, author, tx.cfg_hash);
        vote_against.precheck(fork.snapshot()).map_err(|err| {
            error!("Discarding vote against {:?}: {}", tx, err);
            err
        })?;

        vote_against.save(fork);
        trace!(
            "Put VoteAgainst:{:?} to corresponding cfg votes_by_config_hash table",
            tx
        );

        Ok(())
    }

    fn deploy(
        &self,
        _ctx: TransactionContext,
        arg: transactions::Deploy,
    ) -> Result<(), ExecutionError> {
        info!("Deploying service. {:?}", arg);

        let artifact_spec = arg.get_artifact_spec();

        let dispatcher = self.get_dispatcher_mut();
        dispatcher.start_deploy(artifact_spec).map_err(|err| {
            error!("Service instance deploy failed: {:?}", err);
            ServiceError::DeployError(err)
        })?;

        // TODO add result into deployable (to check deploy status in before_commit).

        Ok(())
    }

    fn init(
        &self,
        mut ctx: TransactionContext,
        arg: transactions::Init,
    ) -> Result<(), ExecutionError> {
        let artifact_spec = arg.get_artifact_spec();

        let dispatcher = self.get_dispatcher_mut();

        let instance_id = self
            .assign_service_id(ctx.fork(), &arg.instance_name)
            .ok_or(ServiceError::ServiceInstanceNameInUse)?;

        let constructor = ServiceConstructor {
            instance_id,
            data: arg.constructor_data,
        };

        info!(
            "Initializing service. Name: {}, id: {}",
            arg.instance_name, instance_id
        );

        dispatcher
            .init_service(ctx.env_context(), artifact_spec, &constructor)
            .map_err(|err| {
                error!("Service instance initialization failed: {:?}", err);
                ServiceError::InitError(err)
            })?;

        Ok(())
    }
}

impl_service_dispatcher!(ConfigurationServiceImpl, ConfigurationService);

impl Service for ConfigurationServiceImpl {
    fn state_hash(&self, snapshot: &dyn Snapshot) -> Vec<Hash> {
        ConfigurationSchema::new(snapshot).state_hash()
    }
}
#[derive(Debug)]
pub struct ConfigurationServiceFactory {
    dispatcher: *mut Dispatcher,
}

impl ConfigurationServiceFactory {
    pub const BUILTIN_ID: ServiceInstanceId = 0;
    pub const BUILTIN_NAME: &'static str = "config";

    pub fn new(dispatcher: &mut Dispatcher) -> Self {
        Self {
            dispatcher: &mut *dispatcher,
        }
    }
}

impl ServiceFactory for ConfigurationServiceFactory {
    fn artifact(&self) -> RustArtifactSpec {
        artifact_spec()
    }

    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(ConfigurationServiceImpl {
            dispatcher: self.dispatcher,
        })
    }
}
