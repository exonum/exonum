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

use criterion::{black_box, Bencher, Criterion};
use exonum::{
    crypto::{self, CryptoHash},
    messages::{Message, RawTransaction, ServiceTransaction, Signed},
};
use exonum_merkledb::BinaryValue;
use rand::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

use std::fmt::Debug;

const CHUNK_SIZE: usize = 64;
const SEED: [u8; 16] = [100; 16];

fn gen_bytes_data() -> Vec<u8> {
    let mut rng = XorShiftRng::from_seed(SEED);
    let mut v = vec![0; CHUNK_SIZE];
    rng.fill_bytes(&mut v);
    v
}

fn signed_raw_transaction() -> Signed<RawTransaction> {
    let keypair = crypto::gen_keypair();
    let payload = gen_bytes_data();
    Message::sign_transaction(
        ServiceTransaction::from_raw_unchecked(10, payload),
        12,
        keypair.0,
        &keypair.1,
    )
}

fn bench_binary_value<F, V>(c: &mut Criterion, name: &str, f: F)
where
    F: Fn() -> V + 'static + Clone + Copy,
    V: BinaryValue + CryptoHash + PartialEq + Debug,
{
    // Checks that binary value is correct.
    let val = f();
    let bytes = val.to_bytes();
    let val2 = V::from_bytes(bytes.into()).unwrap();
    assert_eq!(val, val2);
    // Runs benchmarks.
    c.bench_function(
        &format!("messages/{}/to_bytes", name),
        move |b: &mut Bencher| {
            b.iter_with_setup(f, |data| black_box(data.to_bytes()));
        },
    );
    c.bench_function(
        &format!("messages/{}/into_bytes", name),
        move |b: &mut Bencher| {
            b.iter_with_setup(f, |data| black_box(data.into_bytes()));
        },
    );
    c.bench_function(
        &format!("messages/{}/from_bytes", name),
        move |b: &mut Bencher| {
            b.iter_with_setup(
                || {
                    let val = f();
                    val.to_bytes().into()
                },
                |bytes| black_box(V::from_bytes(bytes).unwrap()),
            );
        },
    );
    c.bench_function(
        &format!("messages/{}/object_hash", name),
        move |b: &mut Bencher| {
            b.iter_with_setup(f, |data| black_box(data.hash()));
        },
    );
}

pub fn bench_messages(c: &mut Criterion) {
    exonum_crypto::init();
    bench_binary_value(c, "signed_any_tx", signed_raw_transaction);
}
