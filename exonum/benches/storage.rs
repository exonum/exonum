#![feature(test)]
extern crate test;
extern crate rand;
extern crate tempdir;
extern crate exonum;

#[cfg(all(test, feature = "long_benchmarks"))]
mod tests {
    use std::collections::HashSet;
    use test::Bencher;
    use rand::{Rng, thread_rng, XorShiftRng, SeedableRng};
    use tempdir::TempDir;
    use exonum::storage::{Database, MemoryDB, RocksDB, RocksDBOptions};
    use exonum::storage::{ProofMapIndex, ProofListIndex};
    use exonum::storage::proof_map_index::KEY_SIZE;

    fn generate_random_kv(len: usize) -> Vec<([u8; KEY_SIZE], Vec<u8>)> {
        let mut rng = thread_rng();
        let mut exists_keys = HashSet::new();
        let mut base = [0; KEY_SIZE];
        rng.fill_bytes(&mut base);
        let base = base;

        let kv_generator = |_| {
            let mut v = vec![0; 8];

            // Generate only unique keys
            let mut k = base.clone();
            let byte: usize = rng.gen_range(0, 31);
            k[byte] = rng.gen::<u8>();

            rng.fill_bytes(&mut v);
            while exists_keys.contains(&k) {
                rng.fill_bytes(&mut k);
            }
            exists_keys.insert(k.clone());
            (k, v)
        };

        (0..len).map(kv_generator).collect::<Vec<_>>()
    }

    fn merkle_table_insertion<T: Database>(b: &mut Bencher, db: &T) {
        let mut rng = XorShiftRng::from_seed([192, 168, 56, 1]);
        let mut storage = db.fork();
        let mut table = ProofListIndex::new(vec![123], &mut storage);
        
        b.iter(|| {
            let v_generator = |_| {
                let mut chunk = vec![0; 10];
                rng.fill_bytes(&mut chunk);
                chunk
            };

            for item in (0..1000).map(v_generator) {
                table.push(item);
            }
        });
    }

    fn merkle_patricia_table_insertion<T: Database>(b: &mut Bencher, db: &T) {
        let data = generate_random_kv(200);
        let mut storage = db.fork();
        let mut table = ProofMapIndex::new(vec![123], &mut storage);

        b.iter(|| for item in &data {
                   table.put(&item.0, item.1.clone());
               });
    }

    fn merkle_patricia_table_insertion_fork<T: Database>(b: &mut Bencher, db: &T) {
        let data = generate_random_kv(200);

        b.iter(|| {
            let patch;
            {
                let mut fork = db.fork();
                {
                    let mut table = ProofMapIndex::new(vec![234], &mut fork);
                    for item in &data {
                        table.put(&item.0, item.1.clone());
                    }
                }
                patch = fork.into_patch();
            }
            db.fork().merge(patch);
        });
    }

    fn merkle_patricia_table_insertion_large_map<T: Database>(b: &mut Bencher, db: &T) {
        let data = generate_random_kv(200);
        let mut storage = db.fork();
        let mut table = ProofMapIndex::new(vec![134], &mut storage);

        for item in &data {
            table.put(&item.0, item.1.clone());
        }

        b.iter(|| for item in &data {
                   table.put(&item.0, item.1.clone());
               });
    }

    fn create_rocksdb(tempdir: &TempDir) -> RocksDB {
        let mut options = RocksDBOptions::default();
        options.create_if_missing(true);
        let db = RocksDB::open(tempdir.path(), options).unwrap();
        db
}
 
    #[bench]
    fn bench_merkle_table_append_memorydb(b: &mut Bencher) {
        let db = MemoryDB::new();
        merkle_table_insertion(b, &db);
    }

    #[bench]
    fn bench_merkle_table_append_rocksdb(b: &mut Bencher) {
        let tempdir = TempDir::new("exonum").unwrap();
        let db = create_rocksdb(&tempdir);
        merkle_table_insertion(b, &db);
    }

    #[bench]
    fn bench_merkle_patricia_table_insertion_memorydb(b: &mut Bencher) {
        let db = MemoryDB::new();
        merkle_patricia_table_insertion(b, &db);
    }

    #[bench]
    fn bench_merkle_patricia_table_insertion_rocksdb(b: &mut Bencher) {
        let tempdir = TempDir::new("exonum").unwrap();
        let db = create_rocksdb(&tempdir);
        merkle_patricia_table_insertion(b, &db);
    }

    #[bench]
    fn bench_merkle_patricia_table_insertion_fork_rocksdb(b: &mut Bencher) {
        let tempdir = TempDir::new("exonum").unwrap();
        let db = create_rocksdb(&tempdir);
        merkle_patricia_table_insertion_fork(b, &db);
    }   

    #[bench]
    fn long_bench_merkle_patricia_table_insertion_memorydb(b: &mut Bencher) {
        let db = MemoryDB::new();
        merkle_patricia_table_insertion_large_map(b, &db);
    }
    
    #[bench]
    fn long_bench_merkle_patricia_table_insertion_rocksdb(b: &mut Bencher) {
        let tempdir = TempDir::new("exonum").unwrap();
        let db = create_rocksdb(&tempdir);
        merkle_patricia_table_insertion_large_map(b, &db);
    }
}
