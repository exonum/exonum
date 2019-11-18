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
    blockchain::ExecutionError,
    helpers::Height,
    runtime::{
        rust::{AfterCommitContext, CallContext, Service},
        InstanceId,
    },
};
use exonum_derive::*;
use exonum_proto::ProtobufConvert;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

pub const SERVICE_ID: InstanceId = 512;
pub const SERVICE_NAME: &str = "after-commit";

#[derive(
    Serialize, Deserialize, Clone, Debug, PartialEq, ProtobufConvert, BinaryValue, ObjectHash,
)]
#[protobuf_convert(source = "proto::TxAfterCommit")]
pub struct TxAfterCommit {
    pub height: Height,
}

#[exonum_interface]
pub trait AfterCommitInterface {
    fn handle_after_commit(
        &self,
        context: CallContext<'_>,
        arg: TxAfterCommit,
    ) -> Result<(), ExecutionError>;
}

impl TxAfterCommit {
    pub fn new(height: Height) -> Self {
        Self { height }
    }
}

#[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
#[service_factory(
    artifact_name = "after-commit",
    artifact_version = "1.0.0",
    proto_sources = "crate::proto",
    service_constructor = "Self::new_instance"
)]
#[service_dispatcher(implements("AfterCommitInterface"))]
pub struct AfterCommitService {
    counter: Arc<AtomicUsize>,
}

impl AfterCommitInterface for AfterCommitService {
    fn handle_after_commit(
        &self,
        _context: CallContext<'_>,
        _arg: TxAfterCommit,
    ) -> Result<(), ExecutionError> {
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

    pub fn new_instance(&self) -> Box<dyn Service> {
        Box::new(self.clone())
    }
}

impl Service for AfterCommitService {
    fn after_commit(&self, context: AfterCommitContext<'_>) {
        self.counter.fetch_add(1, Ordering::SeqCst);
        let tx = TxAfterCommit::new(context.height());
        context.broadcast_transaction(tx);
    }
}
