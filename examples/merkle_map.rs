extern crate da;
extern crate env_logger;
extern crate rand;

use std::path::Path;

use rand::{SeedableRng, XorShiftRng, Rng};

use da::storage::{LevelDB, LevelDBOptions};
use da::storage::{Map, MapExt};

/// usage 
/// path  - Directory where database is situated
/// count - Total amount of data items to write
/// data_len - Length of data chunk
/// seed - seed for rng

fn main() {
    ::std::env::set_var("RUST_LOG", "da=info");

    let mut args = ::std::env::args();
    args.next();

    let path = args.next().unwrap();
    let count: usize = args.next().unwrap().parse().unwrap();
    let data_len: usize = args.next().unwrap().parse().unwrap();
    let seed_part: u32 = args.next().unwrap().parse().unwrap();
    // TODO get them from command line
    let prefix = vec![1];
    let seed = [seed_part, 168, 56, 1];

    let mut rng = XorShiftRng::from_seed(seed);
    let kv_generator = |_| {
        let mut v = vec![0; data_len];
        let mut k: Vec<u8> = vec![0; 32];

        rng.fill_bytes(&mut v);
        rng.fill_bytes(&mut k);
        (k, v)
    };

    let mut options = LevelDBOptions::new();
    options.create_if_missing = true;
    let mut db = LevelDB::new(&Path::new(&path), options).unwrap();

    let mut map = db.merkle_map(prefix); 
    for item in (0..count).map(kv_generator) {
        map.put(&item.0, item.1.clone()).unwrap();
    }
}
