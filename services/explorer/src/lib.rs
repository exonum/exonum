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

//! Exonum explorer service.

use exonum::{
    merkledb::ObjectHash,
    runtime::rust::{api::ServiceApiBuilder, AfterCommitContext, DefaultInstance, Service},
};
use exonum_derive::*;

mod api;
mod websocket;

pub use crate::websocket::Notification;

use crate::{api::ExplorerApi, websocket::SharedState};

#[derive(Debug, Default, ServiceDispatcher)]
pub struct ExplorerService {
    shared_state: SharedState,
}

impl Service for ExplorerService {
    fn after_commit(&self, context: AfterCommitContext<'_>) {
        let block_hash = context.data().for_core().last_block().object_hash();
        self.shared_state.broadcast_block(block_hash);
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        // FIXME: use custom prefix for the service API
        ExplorerApi::new(builder.blockchain().to_owned())
            .wire(self.shared_state.get_ref(), builder.public_scope());
    }
}

#[derive(Debug, Clone, Copy, ServiceFactory)]
#[service_factory(service_constructor = "Self::new_instance")]
pub struct ExplorerFactory;

impl ExplorerFactory {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(ExplorerService::default())
    }
}

impl DefaultInstance for ExplorerFactory {
    const INSTANCE_ID: u32 = 2;
    const INSTANCE_NAME: &'static str = "explorer";
}
