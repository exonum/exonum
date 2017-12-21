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

extern crate cryptocurrency;
extern crate exonum;

use exonum::node::Node;
use exonum::storage::MemoryDB;

use cryptocurrency::{CurrencyService, node_config};

fn main() {
    exonum::helpers::init_logger().unwrap();

    println!("Creating in-memory database...");
    let node = Node::new(
        Box::new(MemoryDB::new()),
        vec![Box::new(CurrencyService)],
        node_config(),
    );
    println!("Starting a single node...");
    println!("Blockchain is ready for transactions!");
    node.run().unwrap();
}
