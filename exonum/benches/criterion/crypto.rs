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

use criterion::{
    AxisScale, Bencher, Criterion, ParameterizedBenchmark, PlotConfiguration, Throughput,
};
use exonum::crypto::{gen_keypair, hash, sign, verify};
use num::pow::pow;

fn bench_sign(b: &mut Bencher, &count: &usize) {
    let (_, secret_key) = gen_keypair();
    let data = (0..count).map(|x| (x % 255) as u8).collect::<Vec<u8>>();
    b.iter(|| sign(&data, &secret_key))
}

fn bench_verify(b: &mut Bencher, &count: &usize) {
    let (public_key, secret_key) = gen_keypair();
    let data = (0..count).map(|x| (x % 255) as u8).collect::<Vec<u8>>();
    let signature = sign(&data, &secret_key);
    b.iter(|| verify(&signature, &data, &public_key))
}

fn bench_hash(b: &mut Bencher, &count: &usize) {
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
        ParameterizedBenchmark::new("hash", bench_hash, (6..16).map(|i| pow(2, i)))
            .throughput(|s| Throughput::Bytes(*s as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
    c.bench(
        "sign",
        ParameterizedBenchmark::new("sign", bench_sign, (6..16).map(|i| pow(2, i)))
            .throughput(|s| Throughput::Bytes(*s as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
    c.bench(
        "verify",
        ParameterizedBenchmark::new("verify", bench_verify, (6..16).map(|i| pow(2, i)))
            .throughput(|s| Throughput::Bytes(*s as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
}
