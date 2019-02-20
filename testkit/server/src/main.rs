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

use exonum_cryptocurrency::service::CurrencyService;
use exonum_testkit::TestKitBuilder;

fn main() {
    exonum::helpers::init_logger().unwrap();

    TestKitBuilder::validator()
        .with_service(CurrencyService)
        .serve(
            "0.0.0.0:8000".parse().unwrap(),
            "0.0.0.0:9000".parse().unwrap(),
        );
}
