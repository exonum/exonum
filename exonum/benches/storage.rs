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

#![feature(test)]

extern crate test;
extern crate rand;
extern crate tempdir;
extern crate exonum;

#[cfg(test)]
#[cfg(feature = "long_benchmarks")]
mod tests {
    use test::Bencher;
    use rand::{SeedableRng, XorShiftRng, Rng};
    use tempdir::TempDir;
    use exonum::storage::{ProofListIndex, ProofMapIndex, Database, MapIndex, Fork, MemoryDB,
                          LevelDB, LevelDBOptions};

    fn generate_random_kv<Gen: Rng>(rng: &mut Gen, len: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
        let kv_generator = |_| {
            let mut v = vec![0; 8];
            let mut k: Vec<u8> = vec![0; 32];

            rng.fill_bytes(&mut v);
            rng.fill_bytes(&mut k);
            (k, v)
        };
        (0..len).map(kv_generator).collect::<Vec<_>>()
    }

    fn merkle_table_insertion<T: Database>(b: &mut Bencher, db: &T) {
        let mut rng = XorShiftRng::from_seed([192, 168, 56, 1]);
        let map = MapIndex::new(vec![123], db);
        let table = ProofListIndex::new(map);
        table.get(0u32).unwrap();
        b.iter(|| {
            let v_generator = |_| {
                let mut chunk = vec![0; 16];
                rng.fill_bytes(&mut chunk);
                chunk
            };

            for item in (0..1000).map(v_generator) {
                table.append(item).unwrap();
            }
        });
    }

    fn merkle_patricia_table_insertion<T: Database>(b: &mut Bencher, db: &T) {
        let mut rng = XorShiftRng::from_seed([192, 168, 56, 1]);
        let data = generate_random_kv(&mut rng, 200);

        let map = MapIndex::new(vec![234], db);
        let table = ProofMapIndex::new(map);
        b.iter(|| for item in &data {
            table.put(&item.0, item.1.clone()).unwrap();
        });
    }

    fn merkle_patricia_table_insertion_fork<T: Database>(b: &mut Bencher, db: &T) {
        let mut rng = XorShiftRng::from_seed([192, 168, 56, 1]);
        let data = generate_random_kv(&mut rng, 200);

        b.iter(|| {
            let patch;
            {
                let fork = db.fork();
                {
                    let map = MapIndex::new(vec![234], &fork);
                    let table = ProofMapIndex::new(map);
                    for item in &data {
                        table.put(&item.0, item.1.clone()).unwrap();
                    }
                }
                patch = fork.into_patch();
            }
            db.merge(&patch).unwrap();
        });
    }

    fn merkle_patricia_table_insertion_large_map<T: Database>(b: &mut Bencher, db: &T) {
        let mut rng = XorShiftRng::from_seed([192, 168, 140, 52]);
        let data = generate_random_kv(&mut rng, 200);
        let kv_generator = |_| {
            let mut v = vec![0; 8];
            let mut k: Vec<u8> = vec![0; 32];

            rng.fill_bytes(&mut v);
            rng.fill_bytes(&mut k);
            (k, v)
        };

        let map = MapIndex::new(vec![134], db);
        let table = ProofMapIndex::new(map);
        for item in (0..10000).map(kv_generator) {
            table.put(&item.0, item.1.clone()).unwrap();
        }

        b.iter(|| for item in &data {
            table.put(&item.0, item.1.clone()).unwrap();
        });
    }

    #[bench]
    fn bench_merkle_table_append_memorydb(b: &mut Bencher) {
        let db = MemoryDB::new();
        merkle_table_insertion(b, db);
    }

    #[bench]
    fn bench_merkle_table_append_leveldb(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let dir = TempDir::new("da_bench").unwrap();
        let db = LevelDB::open(dir.path(), options).unwrap();
        merkle_table_insertion(b, db);
    }

    #[bench]
    fn bench_merkle_patricia_table_insertion_memorydb(b: &mut Bencher) {
        let db = MemoryDB::new();
        merkle_patricia_table_insertion(b, db);
    }

    #[bench]
    fn bench_merkle_patricia_table_insertion_leveldb(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let dir = TempDir::new("da_bench").unwrap();
        let db = LevelDB::open(dir.path(), options).unwrap();
        merkle_patricia_table_insertion(b, db);
    }

    #[bench]
    fn bench_merkle_patricia_table_insertion_fork_leveldb(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let dir = TempDir::new("da_bench").unwrap();
        let db = LevelDB::open(dir.path(), options).unwrap();
        merkle_patricia_table_insertion_fork(b, db);
    }

    #[bench]
    fn long_bench_merkle_patricia_table_insertion_memorydb(b: &mut Bencher) {
        let db = MemoryDB::new();
        merkle_patricia_table_insertion_large_map(b, db);
    }

    #[bench]
    fn long_bench_merkle_patricia_table_insertion_leveldb(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let dir = TempDir::new("da_bench").unwrap();
        let db = LevelDB::open(dir.path(), options).unwrap();
        merkle_patricia_table_insertion_large_map(b, db);
    }

}
