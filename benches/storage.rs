#![feature(test)]

extern crate test;
extern crate rand;
extern crate da;
extern crate tempdir;
extern crate leveldb;

use test::Bencher;
use tempdir::TempDir;
use rand::{SeedableRng, XorShiftRng, Rng};
use leveldb::options::Options;

use da::storage::{MemoryDB, LevelDB};
use da::storage::{List, MapExt};

fn merkle_table_insertion<T: MapExt>(b: &mut Bencher, mut db: T) {
    let mut rng = XorShiftRng::from_seed([192, 168, 56, 1]);
    let mut table = db.merkle_list(vec![220]);
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

#[bench]
fn bench_merkle_table_append_memorydb(b: &mut Bencher) {
    let db = MemoryDB::new();
    merkle_table_insertion(b, db);
}

#[bench]
fn bench_merkle_table_append_leveldb(b: &mut Bencher) {
    let mut options = Options::new();
    options.create_if_missing = true;
    let dir = TempDir::new("da_bench").unwrap();
    let db = LevelDB::new(dir.path(), options).unwrap();
    merkle_table_insertion(b, db);
}
