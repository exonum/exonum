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

use exonum::runtime::rust::{api::ServiceApiBuilder, DefaultInstance, Service};
use exonum_derive::*;

mod api;
mod websocket;

use crate::api::ExplorerApi;

#[derive(Debug, Clone, ServiceFactory, ServiceDispatcher)]
pub struct ExplorerService;

impl Service for ExplorerService {
    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        // FIXME: use custom prefix for the service API
        ExplorerApi::new(builder.blockchain().to_owned()).wire(builder.public_scope());
    }
}

impl DefaultInstance for ExplorerService {
    const INSTANCE_ID: u32 = 2;
    const INSTANCE_NAME: &'static str = "explorer";
}
