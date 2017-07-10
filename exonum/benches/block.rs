#![feature(test)]
#![allow(dead_code)]

extern crate test;
extern crate tempdir;
#[macro_use]
extern crate exonum;

#[cfg(test)]
mod tests {
    use test::Bencher;

    use tempdir::TempDir;

    use exonum::storage::{ProofMapIndex, Database, Fork,
                          LevelDB, LevelDBOptions, LevelDBCache, StorageValue, Patch};

    use std::collections::BTreeMap;
    use exonum::blockchain::{Blockchain, Transaction};
    use exonum::crypto::{gen_keypair, Hash, PublicKey, SecretKey};
    use exonum::messages::Message;


    fn execute_timestamping(db: Box<Database>, b: &mut Bencher) {
        let mut blockchain = Blockchain::new(db, Vec::new());

        message! {
            struct Tx {
                const TYPE = 1;
                const ID = 0;
                const SIZE = 64;

                field from: &PublicKey [00 => 32]
                field data: &Hash      [32 => 64]
            }
        }

        impl Transaction for Tx {
            fn verify(&self) -> bool {
                self.verify_signature(self.from())
            }

            fn execute(&self, _: &mut Fork) {}
        }

        fn prepare_txs(height: u64, count: u64) -> (Vec<Hash>, BTreeMap<Hash, Box<Transaction>>) {
            let (pub_key, sec_key) = gen_keypair();
            let mut txs = Vec::new();
            let mut pool = BTreeMap::new();
            for i in (height * count)..((height + 1) * count) {
                let tx = Tx::new(&pub_key, &i.hash(), &sec_key);
                let tx_hash = Transaction::hash(&tx);
                txs.push(tx_hash);
                pool.insert(tx_hash, Box::new(tx) as Box<Transaction>);
            }
            (txs, pool)
        }

        fn execute_block(blockchain: &Blockchain,
                         height: u64,
                         txs: &[Hash],
                         pool: &HashMap<Hash, Box<Transaction>>) -> Patch {
            blockchain.create_patch(0, height, txs, pool).1
        }

        for i in 0..100 {
            let (txs, pool) = prepare_txs(i, 1000);
            let patch = execute_block(&blockchain, i, &txs, &pool);
            blockchain.merge(patch).unwrap();
        }

        let (txs, pool) = prepare_txs(100, 1000);

        b.iter(|| {
           execute_block(&blockchain, 100, &txs, &pool)
        });
    }

    fn execute_cryptocurrency(db: Box<Database>, b: &mut Bencher) {
        let mut blockchain = Blockchain::new(db, Vec::new());

        message! {
            struct Tx {
                const TYPE = 1;
                const ID = 0;
                const SIZE = 64;

                field from: &PublicKey [00 => 32]
                field to:   &PublicKey [32 => 64]
            }
        }

        impl Transaction for Tx {
            fn verify(&self) -> bool {
                self.verify_signature(self.from())
            }

            fn execute(&self, view: &mut Fork) {
                let mut index = ProofMapIndex::new(vec![1], view);
                let from_balance = index.get(self.from()).unwrap_or(0u64);
                let to_balance = index.get(self.to()).unwrap_or(0u64);
                index.put(self.from(), from_balance - 1);
                index.put(self.to(), to_balance + 1);
            }
        }

        let mut keys = Vec::new();

        for _ in 0..10_000 {
            keys.push(gen_keypair());
        }

        fn prepare_txs(height: u64, count: u64, keys: &[(PublicKey, SecretKey)])
            -> (Vec<Hash>, BTreeMap<Hash, Box<Transaction>>)
        {
            let mut txs = Vec::new();
            let mut pool = BTreeMap::new();
            for i in (height * count)..((height + 1) * count) {
                let tx = Tx::new(&keys[i as usize % 10_000].0,
                                 &keys[(i as usize + 3_456) % 10_000].0,
                                 &keys[i as usize % 10_000].1);
                let tx_hash = Transaction::hash(&tx);
                txs.push(tx_hash);
                pool.insert(tx_hash, Box::new(tx) as Box<Transaction>);
            }
            (txs, pool)
        }

        fn execute_block(blockchain: &Blockchain,
                         height: u64,
                         txs: &[Hash],
                         pool: &BTreeMap<Hash, Box<Transaction>>) -> Patch {
            blockchain.create_patch(0, height, txs, pool).1
        }

        for i in 0..100 {
            let (txs, pool) = prepare_txs(i, 1000, &keys);
            let patch = execute_block(&blockchain, i, &txs, &pool);
            blockchain.merge(patch).unwrap();
        }

        let (txs, pool) = prepare_txs(100, 1000, &keys);

        b.iter(|| {
           execute_block(&blockchain, 100, &txs, &pool)
        });
    }

    #[bench]
    fn bench_execute_block_timestamping_leveldb(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let path = TempDir::new("exonum").unwrap();
        let db = Box::new(LevelDB::open(path.path(), options).unwrap()) as Box<Database>;
        execute_timestamping(db, b)
    }


    #[bench]
    fn bench_execute_block_timestamping_leveldb_cache(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        options.cache = Some(LevelDBCache::new(100_000_000));
        let path = TempDir::new("exonum").unwrap();
        let db = Box::new(LevelDB::open(path.path(), options).unwrap()) as Box<Database>;
        execute_timestamping(db, b)
    }

    #[bench]
    fn bench_execute_block_cryptocurrency_leveldb(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        let path = TempDir::new("exonum").unwrap();
        let db = Box::new(LevelDB::open(path.path(), options).unwrap()) as Box<Database>;
        execute_cryptocurrency(db, b)
    }


    #[bench]
    fn bench_execute_block_cryptocurrency_leveldb_cache(b: &mut Bencher) {
        let mut options = LevelDBOptions::new();
        options.create_if_missing = true;
        options.cache = Some(LevelDBCache::new(100_000_000));
        let path = TempDir::new("exonum").unwrap();
        let db = Box::new(LevelDB::open(path.path(), options).unwrap()) as Box<Database>;
        execute_cryptocurrency(db, b)
    }
}
