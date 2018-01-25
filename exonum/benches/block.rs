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
#![allow(dead_code)]

#[macro_use]
extern crate exonum;
extern crate futures;
extern crate tempdir;
extern crate test;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempdir::TempDir;
    use futures::sync::mpsc;
    use test::Bencher;
    use exonum::storage::{Database, Fork, Patch, ProofMapIndex, StorageValue, RocksDB,
                          RocksDBOptions};
    use exonum::blockchain::{Blockchain, Transaction};
    use exonum::crypto::{gen_keypair, Hash, PublicKey, SecretKey};
    use exonum::messages::Message;
    use exonum::helpers::{Height, ValidatorId};
    use exonum::node::ApiSender;

    fn create_blockchain(db: Box<Database>) -> Blockchain {
        let dummy_channel = mpsc::channel(1);
        let dummy_keypair = (PublicKey::zero(), SecretKey::zero());
        Blockchain::new(
            db,
            Vec::new(),
            dummy_keypair.0,
            dummy_keypair.1,
            ApiSender::new(dummy_channel.0),
        )
    }

    fn execute_timestamping(db: Box<Database>, b: &mut Bencher) {
        let mut blockchain = create_blockchain(db);

        message! {
            struct Tx {
                const TYPE = 1;
                const ID = 0;

                from: &PublicKey,
                data: &Hash,
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

        fn execute_block(
            blockchain: &Blockchain,
            height: u64,
            txs: &[Hash],
            pool: &BTreeMap<Hash, Box<Transaction>>,
        ) -> Patch {
            blockchain
                .create_patch(ValidatorId::zero(), Height(height), txs, pool)
                .1
        }

        for i in 0..100 {
            let (txs, pool) = prepare_txs(i, 1000);
            let patch = execute_block(&blockchain, i, &txs, &pool);
            blockchain.merge(patch).unwrap();
        }

        let (txs, pool) = prepare_txs(100, 1000);

        b.iter(|| execute_block(&blockchain, 100, &txs, &pool));
    }

    fn execute_cryptocurrency(db: Box<Database>, b: &mut Bencher) {
        let mut blockchain = create_blockchain(db);

        message! {
            struct Tx {
                const TYPE = 1;
                const ID = 0;

                from: &PublicKey,
                to: &PublicKey,
            }
        }

        impl Transaction for Tx {
            fn verify(&self) -> bool {
                self.verify_signature(self.from())
            }

            fn execute(&self, view: &mut Fork) {
                let mut index = ProofMapIndex::new("balances_txs", view);
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

        fn prepare_txs(
            height: u64,
            count: u64,
            keys: &[(PublicKey, SecretKey)],
        ) -> (Vec<Hash>, BTreeMap<Hash, Box<Transaction>>) {
            let mut txs = Vec::new();
            let mut pool = BTreeMap::new();
            for i in (height * count)..((height + 1) * count) {
                let tx = Tx::new(
                    &keys[i as usize % 10_000].0,
                    &keys[(i as usize + 3_456) % 10_000].0,
                    &keys[i as usize % 10_000].1,
                );
                let tx_hash = Transaction::hash(&tx);
                txs.push(tx_hash);
                pool.insert(tx_hash, Box::new(tx) as Box<Transaction>);
            }
            (txs, pool)
        }

        fn execute_block(
            blockchain: &Blockchain,
            height: u64,
            txs: &[Hash],
            pool: &BTreeMap<Hash, Box<Transaction>>,
        ) -> Patch {
            blockchain
                .create_patch(ValidatorId::zero(), Height(height), txs, pool)
                .1
        }

        for i in 0..100 {
            let (txs, pool) = prepare_txs(i, 1000, &keys);
            let patch = execute_block(&blockchain, i, &txs, &pool);
            blockchain.merge(patch).unwrap();
        }

        let (txs, pool) = prepare_txs(100, 1000, &keys);

        b.iter(|| execute_block(&blockchain, 100, &txs, &pool));
    }

    fn create_rocksdb(tempdir: &TempDir) -> Box<Database> {
        let mut options = RocksDBOptions::default();
        options.create_if_missing(true);
        let db = Box::new(RocksDB::open(tempdir.path(), &options).unwrap());
        db as Box<Database>
    }

    #[bench]
    fn bench_execute_block_timestamping_rocksdb(b: &mut Bencher) {
        let tempdir = TempDir::new("exonum").unwrap();
        let db = create_rocksdb(&tempdir);
        execute_timestamping(db, b)
    }

    #[bench]
    fn bench_execute_block_cryptocurrency_rocksdb(b: &mut Bencher) {
        let tempdir = TempDir::new("exonum").unwrap();
        let db = create_rocksdb(&tempdir);
        execute_cryptocurrency(db, b)
    }
}
