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

use std::collections::HashSet;

use criterion::{
    AxisScale, Bencher, Criterion, ParameterizedBenchmark, PlotConfiguration, Throughput,
};
use rand::{Rng, RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

use exonum_crypto::Hash;
use exonum_merkledb::{
    proof_map_index::PROOF_MAP_KEY_SIZE as KEY_SIZE, Database, ProofListIndex, ProofMapIndex,
    TemporaryDB,
};

const NAME: &str = "name";
const SAMPLE_SIZE: usize = 10;
const CHUNK_SIZE: usize = 64;
const SEED: [u8; 16] = [100; 16];
const ITEM_COUNTS: [usize; 3] = [1_000, 10_000, 100_000];

fn generate_random_kv(len: usize) -> Vec<(Hash, Vec<u8>)> {
    let mut rng = XorShiftRng::from_seed(SEED);
    let mut exists_keys = HashSet::new();
    let mut base = [0; KEY_SIZE];
    rng.fill_bytes(&mut base);
    let base = base;

    let kv_generator = |_| {
        let mut v = vec![0; CHUNK_SIZE];
        // Generate only unique keys.
        let mut k = base;
        let byte: usize = rng.gen_range(0, 31);
        k[byte] = rng.gen::<u8>();

        rng.fill_bytes(&mut v);
        while exists_keys.contains(&k) {
            rng.fill_bytes(&mut k);
        }
        exists_keys.insert(k);
        (Hash::new(k), v)
    };

    (0..len).map(kv_generator).collect::<Vec<_>>()
}

fn proof_list_append(b: &mut Bencher, len: usize) {
    let mut rng = XorShiftRng::from_seed(SEED);
    let data = (0..len)
        .map(|_| {
            let mut chunk = vec![0; CHUNK_SIZE];
            rng.fill_bytes(&mut chunk);
            chunk
        })
        .collect::<Vec<_>>();

    let db = TemporaryDB::default();
    b.iter_with_setup(
        || db.fork(),
        |mut storage| {
            let mut table = ProofListIndex::new(NAME, &mut storage);
            assert!(table.is_empty());
            for item in &data {
                table.push(item.clone());
            }
        },
    );
}

fn proof_map_insert_without_merge(b: &mut Bencher, len: usize) {
    let data = generate_random_kv(len);
    let db = TemporaryDB::default();
    b.iter_with_setup(
        || db.fork(),
        |mut storage| {
            let mut table = ProofMapIndex::new(NAME, &mut storage);
            assert!(table.keys().next().is_none());
            for item in &data {
                table.put(&item.0, item.1.clone());
            }
        },
    );
}

fn proof_map_insert_with_merge(b: &mut Bencher, len: usize) {
    let data = generate_random_kv(len);
    b.iter_with_setup(TemporaryDB::default, |db| {
        let fork = db.fork();
        {
            let mut table = ProofMapIndex::new(NAME, &mut fork);
            assert!(table.keys().next().is_none());
            for item in &data {
                table.put(&item.0, item.1.clone());
            }
        }
        db.merge(fork.into_patch()).unwrap();
    });
}

fn proof_map_index_build_proofs(b: &mut Bencher, len: usize) {
    let data = generate_random_kv(len);
    let db = TemporaryDB::default();
    let storage = db.fork();
    let mut table = ProofMapIndex::new(NAME, &mut storage);

    for item in &data {
        table.put(&item.0, item.1.clone());
    }
    let table_merkle_root = table.merkle_root();
    let mut proofs = Vec::with_capacity(data.len());

    b.iter(|| {
        proofs.clear();
        proofs.extend(data.iter().map(|item| table.get_proof(item.0)));
    });

    for (i, proof) in proofs.into_iter().enumerate() {
        let checked_proof = proof.check().unwrap();
        assert_eq!(*checked_proof.entries().next().unwrap().1, data[i].1);
        assert_eq!(checked_proof.merkle_root(), table_merkle_root);
    }
}

fn proof_map_index_verify_proofs(b: &mut Bencher, len: usize) {
    let data = generate_random_kv(len);
    let db = TemporaryDB::default();
    let storage = db.fork();
    let mut table = ProofMapIndex::new(NAME, &mut storage);

    for item in &data {
        table.put(&item.0, item.1.clone());
    }
    let table_merkle_root = table.merkle_root();
    let proofs: Vec<_> = data.iter().map(|item| table.get_proof(item.0)).collect();

    b.iter(|| {
        for (i, proof) in proofs.iter().enumerate() {
            let checked_proof = proof.clone().check().unwrap();
            assert_eq!(*checked_proof.entries().next().unwrap().1, data[i].1);
            assert_eq!(checked_proof.merkle_root(), table_merkle_root);
        }
    });
}

fn bench_fn<F>(c: &mut Criterion, name: &str, benchmark: F)
where
    F: Fn(&mut Bencher, usize) + 'static,
{
    let item_counts = ITEM_COUNTS.iter().cloned();
    c.bench(
        name,
        ParameterizedBenchmark::new(
            "items",
            move |b: &mut Bencher, &len: &usize| benchmark(b, len),
            item_counts,
        )
        .throughput(|s| Throughput::Elements(*s as u32))
        .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
        .sample_size(SAMPLE_SIZE),
    );
}

pub fn bench_storage(c: &mut Criterion) {
    exonum_crypto::init();

    bench_fn(c, "proof_list/append", proof_list_append);
    bench_fn(
        c,
        "proof_map/insert/no_merge",
        proof_map_insert_without_merge,
    );
    bench_fn(c, "proof_map/insert/merge", proof_map_insert_with_merge);
    bench_fn(c, "proof_map/proofs/build", proof_map_index_build_proofs);
    bench_fn(
        c,
        "proof_map/proofs/validate",
        proof_map_index_verify_proofs,
    );
}
