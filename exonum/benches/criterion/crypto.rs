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

// Used in bench functions for convenience: we want to be able to pass these functions
// to `ParameterizedBenchmark::new()`.
#![allow(clippy::trivially_copy_pass_by_ref)]

use criterion::{
    AxisScale, Bencher, Criterion, ParameterizedBenchmark, PlotConfiguration, Throughput,
};
use exonum::crypto::{hash, sign, verify, KeyPair};

use std::convert::TryInto;

fn bench_sign(b: &mut Bencher<'_>, &count: &usize) {
    let keys = KeyPair::random();
    let data = (0..count).map(|x| (x % 255) as u8).collect::<Vec<u8>>();
    b.iter(|| sign(&data, keys.secret_key()))
}

fn bench_verify(b: &mut Bencher<'_>, &count: &usize) {
    let keys = KeyPair::random();
    let data = (0..count).map(|x| (x % 255) as u8).collect::<Vec<u8>>();
    let signature = sign(&data, keys.secret_key());
    b.iter(|| verify(&signature, &data, &keys.public_key()))
}

fn bench_hash(b: &mut Bencher<'_>, &count: &usize) {
    let data = (0..count).map(|x| (x % 255) as u8).collect::<Vec<u8>>();
    b.iter(|| hash(&data))
}

pub fn bench_crypto(c: &mut Criterion) {
    ::exonum::crypto::init();

    // Testing crypto functions with different data sizes.
    //
    // 2^6 = 64 - is relatively small message, and our starting test point.
    // 2^16 = 65536 - is relatively big message, and our end point.

    c.bench(
        "hash",
        ParameterizedBenchmark::new("hash", bench_hash, (6..16).map(|i| 1 << i))
            .throughput(|s| Throughput::Bytes((*s).try_into().unwrap()))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
    c.bench(
        "sign",
        ParameterizedBenchmark::new("sign", bench_sign, (6..16).map(|i| 1 << i))
            .throughput(|s| Throughput::Bytes((*s).try_into().unwrap()))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
    c.bench(
        "verify",
        ParameterizedBenchmark::new("verify", bench_verify, (6..16).map(|i| 1 << i))
            .throughput(|s| Throughput::Bytes((*s).try_into().unwrap()))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
}
