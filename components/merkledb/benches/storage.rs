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

use criterion::{
    black_box, AxisScale, Bencher, Criterion, ParameterizedBenchmark, PlotConfiguration, Throughput,
};
use rand::{rngs::StdRng, Rng, RngCore, SeedableRng};
use std::{collections::HashSet, convert::TryInto};

use exonum_crypto::{Hash, HASH_SIZE as KEY_SIZE};
use exonum_merkledb::{access::CopyAccessExt, Database, MapIndex, ObjectHash, TemporaryDB};

const NAME: &str = "name";
const FAMILY: &str = "index_family";
const SAMPLE_SIZE: usize = 10;
const CHUNK_SIZE: usize = 64;
const SEED: [u8; 32] = [100; 32];

#[cfg(all(test, not(feature = "long_benchmarks")))]
const ITEM_COUNTS: [usize; 3] = [1_000, 10_000, 100_000];

#[cfg(all(test, feature = "long_benchmarks"))]
const ITEM_COUNTS: [usize; 4] = [1_000, 10_000, 100_000, 1_000_000];

fn generate_random_kv(len: usize) -> Vec<(Hash, Vec<u8>)> {
    let mut rng: StdRng = SeedableRng::from_seed(SEED);
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

    (0..len).map(kv_generator).collect()
}

fn generate_random_values(len: usize) -> Vec<Vec<u8>> {
    let mut rng: StdRng = SeedableRng::from_seed(SEED);
    (0..len)
        .map(|_| {
            let mut value = vec![0; CHUNK_SIZE];
            rng.fill_bytes(&mut value);
            value
        })
        .collect()
}

fn plain_map_index_insert(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_kv(len);
    b.iter_with_setup(
        || (TemporaryDB::default(), data.clone()),
        |(db, data)| {
            let fork = db.fork();
            {
                let mut table = fork.get_map(NAME);
                for item in data {
                    table.put(&item.0, item.1);
                }
            }
            db.merge_sync(fork.into_patch()).unwrap();
        },
    );
}

fn plain_map_index_with_family_insert(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_kv(len);
    b.iter_with_setup(
        || (TemporaryDB::default(), data.clone()),
        |(db, data)| {
            let fork = db.fork();
            {
                let mut table = fork.get_map((NAME, FAMILY));
                for item in data {
                    table.put(&item.0, item.1);
                }
            }
            db.merge_sync(fork.into_patch()).unwrap();
        },
    );
}

fn plain_map_index_iter(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_kv(len);
    let db = TemporaryDB::default();
    let fork = db.fork();

    {
        let mut table = fork.get_map(NAME);
        assert!(table.keys().next().is_none());
        for item in data {
            table.put(&item.0, item.1);
        }
    }
    db.merge_sync(fork.into_patch()).unwrap();

    b.iter_with_setup(
        || db.snapshot(),
        |snapshot| {
            let index: MapIndex<_, Hash, Vec<u8>> = snapshot.get_map(NAME);
            for (key, value) in &index {
                black_box(key);
                black_box(value);
            }
        },
    );
}

fn plain_map_index_with_family_iter(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_kv(len);
    let db = TemporaryDB::default();
    let fork = db.fork();

    {
        let mut table = fork.get_map((NAME, FAMILY));
        assert!(table.keys().next().is_none());
        for item in data {
            table.put(&item.0, item.1);
        }
    }
    db.merge(fork.into_patch()).unwrap();

    b.iter_with_setup(
        || db.snapshot(),
        |snapshot| {
            let index: MapIndex<_, Hash, Vec<u8>> = snapshot.get_map((NAME, FAMILY));
            for (key, value) in &index {
                black_box(key);
                black_box(value);
            }
        },
    );
}

fn plain_map_index_read(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_kv(len);
    let db = TemporaryDB::default();
    let fork = db.fork();

    {
        let mut table = fork.get_map(NAME);
        assert!(table.keys().next().is_none());
        for item in data.clone() {
            table.put(&item.0, item.1);
        }
    }
    db.merge_sync(fork.into_patch()).unwrap();

    b.iter_with_setup(
        || db.snapshot(),
        |snapshot| {
            let index: MapIndex<_, Hash, Vec<u8>> = snapshot.get_map(NAME);
            for item in &data {
                let value = index.get(&item.0);
                black_box(value);
            }
        },
    );
}

fn plain_map_index_with_family_read(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_kv(len);
    let db = TemporaryDB::default();
    let fork = db.fork();

    {
        let mut table = fork.get_map((NAME, FAMILY));
        assert!(table.keys().next().is_none());
        for item in data.clone() {
            table.put(&item.0, item.1);
        }
    }
    db.merge_sync(fork.into_patch()).unwrap();

    b.iter_with_setup(
        || db.snapshot(),
        |snapshot| {
            let index: MapIndex<_, Hash, Vec<u8>> = snapshot.get_map((NAME, FAMILY));
            for item in &data {
                let value = index.get(&item.0);
                black_box(value);
            }
        },
    );
}

fn proof_list_append(b: &mut Bencher<'_>, len: usize) {
    let mut rng: StdRng = SeedableRng::from_seed(SEED);
    let data = (0..len)
        .map(|_| {
            let mut chunk = vec![0; CHUNK_SIZE];
            rng.fill_bytes(&mut chunk);
            chunk
        })
        .collect::<Vec<_>>();

    let db = TemporaryDB::default();
    b.iter_with_setup(
        || (db.fork(), data.clone()),
        |(fork, data)| {
            let mut table = fork.get_proof_list(NAME);
            assert!(table.is_empty());
            for item in data {
                table.push(item);
            }
        },
    );
}

fn proof_list_extend(b: &mut Bencher<'_>, len: usize) {
    let mut rng: StdRng = SeedableRng::from_seed(SEED);
    let data = (0..len)
        .map(|_| {
            let mut chunk = vec![0; CHUNK_SIZE];
            rng.fill_bytes(&mut chunk);
            chunk
        })
        .collect::<Vec<_>>();

    let db = TemporaryDB::default();
    b.iter_with_setup(
        || (db.fork(), data.clone()),
        |(fork, data)| {
            let mut table = fork.get_proof_list(NAME);
            assert!(table.is_empty());
            table.extend(data);
        },
    );
}

fn proof_map_insert_without_merge(b: &mut Bencher<'_>, len: usize) {
    let db = TemporaryDB::default();
    let data = generate_random_kv(len);
    b.iter_with_setup(
        || (db.fork(), data.clone()),
        |(fork, data)| {
            let mut table = fork.get_proof_map(NAME);
            for item in data {
                table.put(&item.0, item.1);
            }
        },
    );
}

fn proof_map_insert_with_merge(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_kv(len);
    b.iter_with_setup(
        || (TemporaryDB::default(), data.clone()),
        |(db, data)| {
            let fork = db.fork();
            {
                let mut table = fork.get_proof_map(NAME);
                for item in data {
                    table.put(&item.0, item.1);
                }
            }
            db.merge_sync(fork.into_patch()).unwrap();
        },
    );
}

fn proof_list_index_build_proofs(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_values(len);
    let db = TemporaryDB::default();
    let fork = db.fork();
    let mut table = fork.get_proof_list(NAME);

    for item in &data {
        table.push(item.clone());
    }
    let mut proofs = Vec::with_capacity(data.len());

    b.iter(|| {
        proofs.clear();
        proofs.extend((0..len).map(|i| table.get_proof(i as u64)));
    });

    let table_hash = table.object_hash();
    for proof in proofs {
        let checked_proof = proof.check().unwrap();
        assert_eq!(checked_proof.index_hash(), table_hash);
        assert_eq!(checked_proof.entries().len(), 1);
    }
}

fn proof_list_index_verify_proofs(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_values(len);
    let db = TemporaryDB::default();
    let fork = db.fork();
    let mut table = fork.get_proof_list(NAME);

    for item in &data {
        table.push(item.clone());
    }
    let table_hash = table.object_hash();
    let proofs: Vec<_> = (0..len).map(|i| table.get_proof(i as u64)).collect();

    b.iter(|| {
        for proof in &proofs {
            let items = proof.check_against_hash(table_hash).unwrap();
            assert_eq!(items.entries().len(), 1);
        }
    });
}

fn proof_map_index_build_proofs(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_kv(len);
    let db = TemporaryDB::default();
    let fork = db.fork();
    let mut table = fork.get_proof_map(NAME);

    for item in &data {
        table.put(&item.0, item.1.clone());
    }
    let table_hash = table.object_hash();
    let mut proofs = Vec::with_capacity(data.len());

    b.iter(|| {
        proofs.clear();
        proofs.extend(data.iter().map(|item| table.get_proof(item.0)));
    });

    for (i, proof) in proofs.into_iter().enumerate() {
        let checked_proof = proof.check_against_hash(table_hash).unwrap();
        assert_eq!(*checked_proof.entries().next().unwrap().1, data[i].1);
    }
}

fn proof_map_index_verify_proofs(b: &mut Bencher<'_>, len: usize) {
    let data = generate_random_kv(len);
    let db = TemporaryDB::default();
    let fork = db.fork();
    let mut table = fork.get_proof_map(NAME);

    for item in &data {
        table.put(&item.0, item.1.clone());
    }
    let table_hash = table.object_hash();
    let proofs: Vec<_> = data.iter().map(|item| table.get_proof(item.0)).collect();

    b.iter(|| {
        for (i, proof) in proofs.iter().enumerate() {
            let checked_proof = proof.check_against_hash(table_hash).unwrap();
            assert_eq!(*checked_proof.entries().next().unwrap().1, data[i].1);
        }
    });
}

fn bench_fn<F>(c: &mut Criterion, name: &str, benchmark: F)
where
    F: Fn(&mut Bencher<'_>, usize) + 'static,
{
    let item_counts = ITEM_COUNTS.iter().cloned();
    c.bench(
        name,
        ParameterizedBenchmark::new(
            "items",
            move |b: &mut Bencher<'_>, &len: &usize| benchmark(b, len),
            item_counts,
        )
        .throughput(|s| Throughput::Elements((*s).try_into().unwrap()))
        .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic))
        .sample_size(SAMPLE_SIZE),
    );
}

pub fn bench_storage(c: &mut Criterion) {
    exonum_crypto::init();
    // MapIndex
    bench_fn(c, "storage/plain_map/insert", plain_map_index_insert);
    bench_fn(c, "storage/plain_map/iter", plain_map_index_iter);
    bench_fn(
        c,
        "storage/plain_map_with_family/insert",
        plain_map_index_with_family_insert,
    );
    bench_fn(
        c,
        "storage/plain_map_with_family/iter",
        plain_map_index_with_family_iter,
    );
    bench_fn(c, "storage/plain_map/read", plain_map_index_read);
    bench_fn(
        c,
        "storage/plain_map_with_family/read",
        plain_map_index_with_family_read,
    );
    // ProofListIndex
    bench_fn(c, "storage/proof_list/append", proof_list_append);

    bench_fn(c, "storage/proof_list/extend", proof_list_extend);

    bench_fn(
        c,
        "storage/proof_list/proofs/build",
        proof_list_index_build_proofs,
    );
    bench_fn(
        c,
        "storage/proof_list/proofs/validate",
        proof_list_index_verify_proofs,
    );
    // ProofMapIndex
    bench_fn(
        c,
        "storage/proof_map/insert/no_merge",
        proof_map_insert_without_merge,
    );
    bench_fn(
        c,
        "storage/proof_map/insert/merge",
        proof_map_insert_with_merge,
    );
    bench_fn(
        c,
        "storage/proof_map/proofs/build",
        proof_map_index_build_proofs,
    );
    bench_fn(
        c,
        "storage/proof_map/proofs/validate",
        proof_map_index_verify_proofs,
    );
}
