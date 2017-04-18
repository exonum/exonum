#![feature(test)]

extern crate test;
extern crate rand;
extern crate tempdir;
extern crate exonum;

#[cfg(test)]
mod tests {
    use test::Bencher;
    use tempdir::TempDir;
    use rand::{SeedableRng, XorShiftRng, Rng};
    use exonum::storage::{MerkleTable, MerklePatriciaTable, Database, Map, List, MapTable, Fork,
                          MemoryDB, LevelDB, LevelDBOptions};

    fn generate_random_kv<Gen: Rng>(rng: &mut Gen, len: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
        let kv_generator = |_| {
            let mut v = vec![0; 8];
            let mut k: Vec<u8> = vec![0; 32];

            rng.fill_bytes(&mut v);
            rng.fill_bytes(&mut k);
            (k, v)
        };
        (0..len)
            .map(kv_generator)
            .collect::<Vec<_>>()
    }

    fn merkle_table_insertion<T: Database>(b: &mut Bencher, mut db: T) {
        let mut rng = XorShiftRng::from_seed([192, 168, 56, 1]);
        let map = MapTable::new(vec![123], &mut db);
        let table = MerkleTable::new(map);
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

    fn merkle_patricia_table_insertion<T: Database>(b: &mut Bencher, mut db: T) {
        let mut rng = XorShiftRng::from_seed([192, 168, 56, 1]);
        let data = generate_random_kv(&mut rng, 200);

        let map = MapTable::new(vec![234], &mut db);
        let table = MerklePatriciaTable::new(map);
        b.iter(|| {
            for item in &data {
                table.put(&item.0, item.1.clone()).unwrap();
            }
        });
    }

    fn merkle_patricia_table_insertion_fork<T: Database>(b: &mut Bencher, db: T) {
        let mut rng = XorShiftRng::from_seed([192, 168, 56, 1]);
        let data = generate_random_kv(&mut rng, 200);

        b.iter(|| {
            let patch;
            {
                let mut fork = db.fork();
                {
                    let map = MapTable::new(vec![234], &mut fork);
                    let table = MerklePatriciaTable::new(map);
                    for item in &data {
                        table.put(&item.0, item.1.clone()).unwrap();
                    }
                }
                patch = fork.changes();
            }
            db.merge(&patch).unwrap();
        });
    }

    fn merkle_patricia_table_insertion_large_map<T: Database>(b: &mut Bencher, mut db: T) {
        let mut rng = XorShiftRng::from_seed([192, 168, 140, 52]);
        let data = generate_random_kv(&mut rng, 200);
        let kv_generator = |_| {
            let mut v = vec![0; 8];
            let mut k: Vec<u8> = vec![0; 32];

            rng.fill_bytes(&mut v);
            rng.fill_bytes(&mut k);
            (k, v)
        };

        let map = MapTable::new(vec![134], &mut db);
        let table = MerklePatriciaTable::new(map);
        for item in (0..10000).map(kv_generator) {
            table.put(&item.0, item.1.clone()).unwrap();
        }

        b.iter(|| {
            for item in &data {
                table.put(&item.0, item.1.clone()).unwrap();
            }
        });
    }

    #[cfg(feature = "long_benchmarks")]
    #[bench]
    fn bench_merkle_table_append_memorydb(b: &mut Bencher) {
        let db = MemoryDB::new();
        merkle_table_insertion(b, db);
    }

    #[cfg(feature = "long_benchmarks")]
    #[bench]
    fn bench_merkle_table_append_leveldb(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let dir = TempDir::new("da_bench").unwrap();
        let db = LevelDB::new(dir.path(), options).unwrap();
        merkle_table_insertion(b, db);
    }

    #[cfg(feature = "long_benchmarks")]
    #[bench]
    fn bench_merkle_patricia_table_insertion_memorydb(b: &mut Bencher) {
        let db = MemoryDB::new();
        merkle_patricia_table_insertion(b, db);
    }

    #[cfg(feature = "long_benchmarks")]
    #[bench]
    fn bench_merkle_patricia_table_insertion_leveldb(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let dir = TempDir::new("da_bench").unwrap();
        let db = LevelDB::new(dir.path(), options).unwrap();
        merkle_patricia_table_insertion(b, db);
    }

    #[cfg(feature = "long_benchmarks")]
    #[bench]
    fn bench_merkle_patricia_table_insertion_fork_leveldb(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let dir = TempDir::new("da_bench").unwrap();
        let db = LevelDB::new(dir.path(), options).unwrap();
        merkle_patricia_table_insertion_fork(b, db);
    }

    #[cfg(feature = "long_benchmarks")]
    #[bench]
    fn long_bench_merkle_patricia_table_insertion_memorydb(b: &mut Bencher) {
        let db = MemoryDB::new();
        merkle_patricia_table_insertion_large_map(b, db);
    }

    #[cfg(feature = "long_benchmarks")]
    #[bench]
    fn long_bench_merkle_patricia_table_insertion_leveldb(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let dir = TempDir::new("da_bench").unwrap();
        let db = LevelDB::new(dir.path(), options).unwrap();
        merkle_patricia_table_insertion_large_map(b, db);
    }

}
