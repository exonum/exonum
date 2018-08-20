// Copyright 2018 The Exonum Team
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

#[macro_use]
extern crate criterion;
#[macro_use]
extern crate exonum;
extern crate futures;
extern crate num;
extern crate rand;
extern crate tempdir;
extern crate tokio_core;

use block::bench_block;
use criterion::Criterion;
use crypto::bench_crypto;
use storage::bench_storage;
use transactions::bench_verify_transactions;

mod block;
mod crypto;
mod storage;
mod transactions;

criterion_group!(
    benches,
    bench_crypto,
    bench_block,
    bench_storage,
    bench_verify_transactions
);
criterion_main!(benches);
