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

use da::storage::{MerklePatriciaTable, MemoryDB, LevelDB};
use da::storage::{Map, List, MapExt};

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

fn generate_random_data<Gen: Rng>(rng: &mut Gen, len: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
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

fn merkle_patricia_table_insertion<T: Map<[u8], Vec<u8>>>(b: &mut Bencher, map: T) {
    let mut rng = XorShiftRng::from_seed([192, 168, 56, 1]);
    let data = generate_random_data(&mut rng, 200);

    let mut table = MerklePatriciaTable::new(map);
    b.iter(|| {
        for item in &data {
            table.put(&item.0, item.1.clone()).unwrap();
        }
    });
}

fn merkle_patricia_table_insertion_large_map<T: Map<[u8], Vec<u8>>>(b: &mut Bencher, map: T) {
    let mut table = MerklePatriciaTable::new(map);

    let mut rng = XorShiftRng::from_seed([192, 168, 140, 52]);
    let data = generate_random_data(&mut rng, 200);

    let kv_generator = |_| {
        let mut v = vec![0; 8];
        let mut k: Vec<u8> = vec![0; 32];

        rng.fill_bytes(&mut v);
        rng.fill_bytes(&mut k);
        (k, v)
    };
    for item in (0..10000).map(kv_generator) {
        table.put(&item.0, item.1.clone()).unwrap();
    }

    b.iter(|| {
        for item in &data {
            table.put(&item.0, item.1.clone()).unwrap();
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

#[bench]
fn bench_merkle_patricia_table_insertion_memorydb(b: &mut Bencher) {
    let mut db = MemoryDB::new();
    let map = db.map(vec![255]);
    merkle_patricia_table_insertion(b, map);
}

#[bench]
fn bench_merkle_patricia_table_insertion_leveldb(b: &mut Bencher) {
    let mut options = Options::new();
    options.create_if_missing = true;
    let dir = TempDir::new("da_bench").unwrap();
    let mut db = LevelDB::new(dir.path(), options).unwrap();
    let map = db.map(vec![254]);
    merkle_patricia_table_insertion(b, map);
}

#[bench]
fn long_bench_merkle_patricia_table_insertion_memorydb(b: &mut Bencher) {
    let mut db = MemoryDB::new();
    let map = db.map(vec![255]);
    merkle_patricia_table_insertion_large_map(b, map);
}

#[bench]
fn long_bench_merkle_patricia_table_insertion_leveldb(b: &mut Bencher) {
    let mut options = Options::new();
    options.create_if_missing = true;
    let dir = TempDir::new("da_bench").unwrap();
    let mut db = LevelDB::new(dir.path(), options).unwrap();
    let map = db.map(vec![254]);
    merkle_patricia_table_insertion_large_map(b, map);
}
