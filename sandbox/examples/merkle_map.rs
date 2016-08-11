extern crate exonum;
extern crate env_logger;
extern crate rand;
#[macro_use]
extern crate clap;

use std::path::Path;

use rand::{SeedableRng, XorShiftRng, Rng};

use exonum::storage::{LevelDB, LevelDBOptions};
use exonum::storage::{Database, Map, MerklePatriciaTable, MapTable, Patch};

/// usage 
/// path  - Directory where database is situated
/// count - Total amount of data items to write
/// data_len - Length of data chunk
/// seed - seed for rng

fn main() {
    ::std::env::set_var("RUST_LOG", "da=info");

    let matches = clap_app!(merkle_map =>
        (version: "0.1")
        (author: "Aleksey S. <aleksei.sidorov@xdev.re>")
        (about: "Merkle map benchmarking tool")
        (@arg DIR: +required "Sets the input directory to use")
        (@arg count: -c --count +takes_value "Total amount of data items to write")
        (@arg data_len: -l --len +takes_value "Length of data chunk")
        (@arg seed: -s --seed +takes_value "Seed for rng")
        (@arg fork: -f --fork "Use fork to write data in one transaction")
    ).get_matches();    

    let path = matches.value_of("DIR").unwrap();
    let count: usize = matches.value_of("count").unwrap_or("100").parse().unwrap();
    let data_len: usize = matches.value_of("data_len").unwrap_or("64").parse().unwrap();
    let seed_part: u32 = matches.value_of("seed").unwrap_or("0").parse().unwrap();
    let use_fork: bool = matches.is_present("fork");
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
    if use_fork {
        let patch;
        {
            let mut fork = db.fork();
            {
                let mut map = MerklePatriciaTable::new(MapTable::new(prefix, &mut fork));        
                for item in (0..count).map(kv_generator) {
                    map.put(&item.0, item.1.clone()).unwrap();
                }
            }
            patch = Patch::from(fork);
        }
        db.merge(patch).unwrap();
    } else {
        let mut map = MerklePatriciaTable::new(MapTable::new(prefix, &mut db));  
        for item in (0..count).map(kv_generator) {
            map.put(&item.0, item.1.clone()).unwrap();
        }
    }
}
