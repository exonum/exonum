// Copyright 2020 The Exonum Team
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

use exonum_cli::NodeBuilder;
use exonum_cryptocurrency_advanced as cryptocurrency;
use exonum_rust_runtime::ServiceFactory;

fn main() -> Result<(), failure::Error> {
    exonum::helpers::init_logger().unwrap();
    NodeBuilder::new()
        .with_service(cryptocurrency::CryptocurrencyService)
        // Starts cryptocurrency instance with given id and name
        // immediately after genesis block creation.
        .with_default_instance(
            cryptocurrency::CryptocurrencyService
                .artifact_id()
                .into_default_instance(101, "cryptocurrency"),
        )
        .run()
}
