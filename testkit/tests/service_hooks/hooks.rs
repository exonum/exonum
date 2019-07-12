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

//! A special service which generates transactions on `after_commit` events.
use super::proto;
use exonum::{
    blockchain::ExecutionResult,
    helpers::Height,
    impl_service_dispatcher,
    runtime::{
        rust::{AfterCommitContext, RustArtifactId, Service, ServiceFactory, TransactionContext},
        ArtifactInfo, ServiceInstanceId,
    },
};
use exonum_derive::{service_interface, ProtobufConvert};

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

pub const SERVICE_ID: ServiceInstanceId = 512;
pub const SERVICE_NAME: &str = "after-commit";

#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, PartialEq)]
#[exonum(pb = "proto::TxAfterCommit")]
pub struct TxAfterCommit {
    pub height: Height,
}

#[service_interface]
pub trait AfterCommitInterface {
    fn handle_after_commit(
        &self,
        context: TransactionContext,
        arg: TxAfterCommit,
    ) -> ExecutionResult;
}

impl TxAfterCommit {
    pub fn new(height: Height) -> Self {
        Self { height }
    }
}

#[derive(Clone, Default, Debug)]
pub struct AfterCommitService {
    counter: Arc<AtomicUsize>,
}

impl AfterCommitInterface for AfterCommitService {
    fn handle_after_commit(
        &self,
        _context: TransactionContext,
        _arg: TxAfterCommit,
    ) -> ExecutionResult {
        Ok(())
    }
}

impl AfterCommitService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn counter(&self) -> usize {
        self.counter.load(Ordering::SeqCst)
    }
}

impl Service for AfterCommitService {
    fn after_commit(&self, context: AfterCommitContext) {
        self.counter.fetch_add(1, Ordering::SeqCst);
        let tx = TxAfterCommit::new(context.height());
        context.broadcast_transaction(tx);
    }
}

impl_service_dispatcher!(AfterCommitService, AfterCommitInterface);

impl ServiceFactory for AfterCommitService {
    fn artifact_id(&self) -> RustArtifactId {
        "after-commit/1.0.0".parse().unwrap()
    }

    fn artifact_info(&self) -> ArtifactInfo {
        ArtifactInfo {
            proto_sources: proto::PROTO_SOURCES.as_ref(),
        }
    }

    fn create_instance(&self) -> Box<dyn Service> {
        Box::new(self.clone())
    }
}
