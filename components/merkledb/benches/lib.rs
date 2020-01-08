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

use criterion::{criterion_group, criterion_main};

use crate::{
    encoding::bench_encoding, schema_patterns::bench_schema_patterns, storage::bench_storage,
    transactions::bench_transactions,
};

mod encoding;
mod schema_patterns;
mod storage;
mod transactions;

criterion_group!(
    benches,
    bench_storage,
    bench_encoding,
    bench_schema_patterns,
    bench_transactions
);
criterion_main!(benches);
