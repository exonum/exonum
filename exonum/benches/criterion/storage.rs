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

const SAMPLE_SIZE: usize = 20;

use criterion::{
    AxisScale, Bencher, Criterion, ParameterizedBenchmark, PlotConfiguration, Throughput,
};
use exonum::{
    crypto::Hash,
    storage::{
        proof_map_index::PROOF_MAP_KEY_SIZE as KEY_SIZE, Database, DbOptions, ProofListIndex,
        ProofMapIndex, RocksDB,
    },
};
use rand::{Rng, RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;
use tempdir::TempDir;

use std::collections::HashSet;

const NAME: &str = "name";

fn generate_random_kv(len: usize) -> Vec<(Hash, Vec<u8>)> {
    let mut rng = XorShiftRng::from_seed([100; 16]);
    let mut exists_keys = HashSet::new();
    let mut base = [0; KEY_SIZE];
    rng.fill_bytes(&mut base);
    let base = base;

    let kv_generator = |_| {
        let mut v = vec![0; 64];

        // Generate only unique keys
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

fn proof_list_append<T: Database>(b: &mut Bencher, db: &T, len: &usize) {
    let mut rng = XorShiftRng::from_seed([200; 16]);
    let mut storage = db.fork();
    let mut table = ProofListIndex::new(NAME, &mut storage);

    b.iter_with_setup(
        || {
            (0..*len)
                .map(|_| {
                    let mut chunk = vec![0; 64];
                    rng.fill_bytes(&mut chunk);
                    chunk
                }).collect::<Vec<_>>()
        },
        |data| {
            for item in data {
                table.push(item);
            }
        },
    );
}

fn proof_map_insert_without_merge<T: Database>(b: &mut Bencher, db: &T, len: &usize) {
    let data = generate_random_kv(*len);
    b.iter_with_setup(
        || db.fork(),
        |mut storage| {
            let mut table = ProofMapIndex::new(NAME, &mut storage);
            for item in &data {
                table.put(&item.0, item.1.clone());
            }
        },
    );
}

fn proof_map_insert_with_merge<T: Database>(b: &mut Bencher, db: &T, len: &usize) {
    let data = generate_random_kv(*len);
    b.iter_with_large_setup(
        || {
            let patch = {
                let mut fork = db.fork();
                {
                    let mut table: ProofMapIndex<_, Hash, Vec<u8>> =
                        ProofMapIndex::new(NAME, &mut fork);
                    table.clear();
                }
                fork.into_patch()
            };
            db.merge(patch).unwrap();
        },
        |_| {
            let patch;
            {
                let mut fork = db.fork();
                {
                    let mut table = ProofMapIndex::new(NAME, &mut fork);
                    for item in &data {
                        table.put(&item.0, item.1.clone());
                    }
                }
                patch = fork.into_patch();
            }
            db.merge(patch).unwrap();
        },
    );
}

fn proof_map_index_build_proofs<T: Database>(b: &mut Bencher, db: &T, len: &usize) {
    let data = generate_random_kv(*len);
    let mut storage = db.fork();
    let mut table = ProofMapIndex::new(NAME, &mut storage);

    for item in &data {
        table.put(&item.0, item.1.clone());
    }
    let table_merkle_root = table.merkle_root();
    let mut proofs = Vec::with_capacity(data.len());

    b.iter(|| {
        proofs = Vec::with_capacity(data.len());
        proofs.extend(data.iter().map(|item| table.get_proof(item.0)));
    });

    for (i, proof) in proofs.into_iter().enumerate() {
        let checked_proof = proof.check().unwrap();
        assert_eq!(*checked_proof.entries().next().unwrap().1, data[i].1);
        assert_eq!(checked_proof.merkle_root(), table_merkle_root);
    }
}

fn proof_map_index_verify_proofs<T: Database>(b: &mut Bencher, db: &T, len: &usize) {
    let data = generate_random_kv(*len);
    let mut storage = db.fork();
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

fn create_rocksdb(tempdir: &TempDir) -> RocksDB {
    let options = DbOptions::default();
    RocksDB::open(tempdir.path(), &options).unwrap()
}

fn bench_proof_list_append(b: &mut Bencher, len: &usize) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    proof_list_append(b, &db, len);
}

fn bench_proof_map_insert_without_merge(b: &mut Bencher, len: &usize) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    proof_map_insert_without_merge(b, &db, len);
}

fn bench_proof_map_insert_with_merge(b: &mut Bencher, len: &usize) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    proof_map_insert_with_merge(b, &db, len);
}

fn bench_proof_map_index_build_proofs(b: &mut Bencher, len: &usize) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    proof_map_index_build_proofs(b, &db, len);
}

fn bench_proof_map_index_validate_proofs(b: &mut Bencher, len: &usize) {
    let tempdir = TempDir::new("exonum").unwrap();
    let db = create_rocksdb(&tempdir);
    proof_map_index_verify_proofs(b, &db, len);
}

pub fn bench_storage(c: &mut Criterion) {
    ::exonum::crypto::init();

    let item_counts = vec![100, 1_000, 10_000];

    c.bench(
        "storage/proof_list/append",
        ParameterizedBenchmark::new("items", bench_proof_list_append, item_counts.clone())
            .throughput(|s| Throughput::Elements(*s as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
            .sample_size(SAMPLE_SIZE),
    );
    c.bench(
        "storage/proof_map/insert/no_merge",
        ParameterizedBenchmark::new(
            "items",
            bench_proof_map_insert_without_merge,
            item_counts.clone(),
        ).throughput(|s| Throughput::Elements(*s as u32))
        .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
        .sample_size(SAMPLE_SIZE),
    );
    c.bench(
        "storage/proof_map/insert/merge",
        ParameterizedBenchmark::new(
            "items",
            bench_proof_map_insert_with_merge,
            item_counts.clone(),
        ).throughput(|s| Throughput::Elements(*s as u32))
        .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
        .sample_size(SAMPLE_SIZE),
    );
    c.bench(
        "storage/proof_map/proofs/build",
        ParameterizedBenchmark::new(
            "items",
            bench_proof_map_index_build_proofs,
            item_counts.clone(),
        ).throughput(|s| Throughput::Elements(*s as u32))
        .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
        .sample_size(SAMPLE_SIZE),
    );
    c.bench(
        "storage/proof_map/proofs/validate",
        ParameterizedBenchmark::new(
            "items",
            bench_proof_map_index_validate_proofs,
            item_counts.clone(),
        ).throughput(|s| Throughput::Elements(*s as u32))
        .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
        .sample_size(SAMPLE_SIZE),
    );
}
