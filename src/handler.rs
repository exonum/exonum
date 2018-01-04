// Copyright 2017 The Exonum Team
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

use std::sync::{Arc, RwLock};
use std::num::ParseIntError;

use bodyparser;
use exonum::api::ApiError;
use exonum::crypto;
use exonum::explorer::BlockchainExplorer;
use iron::prelude::*;
use iron::headers::ContentType;
use iron::modifiers::Header;
use iron::status::Status;
use router::Router;
use serde::Serialize;
use serde_json;

use super::{TestKit, TestNetworkConfiguration};

///  Creates an Iron handler for processing testkit-specific HTTP requests.
pub fn create_testkit_handler(testkit: &Arc<RwLock<TestKit>>) -> Router {
    let mut router = Router::new();

    let clone = Arc::clone(testkit);
    router.get(
        "v1/status",
        move |req: &mut Request| {
            clone
                .read()
                .expect("Cannot acquire ref to testkit")
                .handle_status(req)
        },
        "status",
    );

    let clone = Arc::clone(testkit);
    router.post(
        "v1/blocks",
        move |req: &mut Request| {
            clone
                .write()
                .expect("Cannot acquire mutable ref to testkit")
                .handle_create_block(req)
        },
        "create_block",
    );

    let clone = Arc::clone(testkit);
    router.delete(
        "v1/blocks/:height",
        move |req: &mut Request| {
            clone
                .write()
                .expect("Cannot acquire mutable ref to testkit")
                .handle_rollback(req)
        },
        "rollback",
    );

    router
}

fn ok_response<S: Serialize>(response: &S) -> IronResult<Response> {
    Ok(Response::with((
        serde_json::to_string(response).unwrap(),
        Status::Ok,
        Header(ContentType::json()),
    )))
}

trait TestKitHandler {
    fn handle_status(&self, req: &mut Request) -> IronResult<Response>;
    fn handle_create_block(&mut self, req: &mut Request) -> IronResult<Response>;
    fn handle_rollback(&mut self, req: &mut Request) -> IronResult<Response>;
}

impl TestKitHandler for TestKit {
    fn handle_status(&self, _: &mut Request) -> IronResult<Response> {
        use exonum::helpers::Height;

        #[derive(Debug, Serialize, Deserialize)]
        struct TestKitStatus {
            height: Height,
            configuration: TestNetworkConfiguration,
            next_configuration: Option<TestNetworkConfiguration>,
        }

        let status = TestKitStatus {
            height: self.height(),
            configuration: self.configuration_change_proposal(),
            next_configuration: self.next_configuration().cloned(),
        };
        ok_response(&status)
    }

    fn handle_create_block(&mut self, req: &mut Request) -> IronResult<Response> {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        struct CreateBlockRequest {
            tx_hashes: Option<Vec<crypto::Hash>>,
        }

        match req.get::<bodyparser::Struct<CreateBlockRequest>>() {
            Ok(Some(req)) => {
                if let Some(tx_hashes) = req.tx_hashes {
                    self.create_block_with_tx_hashes(&tx_hashes);
                } else {
                    self.create_block();
                }

                let explorer = BlockchainExplorer::new(&self.blockchain);
                let block_info = explorer.block_info(self.height());
                ok_response(&block_info)
            }
            Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
            Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
        }
    }

    fn handle_rollback(&mut self, req: &mut Request) -> IronResult<Response> {
        let params = req.extensions.get::<Router>().unwrap();

        let height: u64 = match params.find("height") {
            Some(height_str) => {
                height_str.parse().map_err(|e: ParseIntError| {
                    ApiError::IncorrectRequest(Box::new(e))
                })?
            }
            None => {
                Err(ApiError::IncorrectRequest(
                    "Required request parameter is missing: height".into(),
                ))?
            }
        };
        if height == 0 {
            Err(ApiError::IncorrectRequest(
                "Cannot rollback past genesis block".into(),
            ))?;
        }

        if self.height().0 >= height {
            let rollback_blocks = (self.height().0 - height + 1) as usize;
            self.rollback(rollback_blocks);
        }
        let explorer = BlockchainExplorer::new(&self.blockchain);
        let block_info = explorer.block_info(self.height());
        ok_response(&block_info)
    }
}
