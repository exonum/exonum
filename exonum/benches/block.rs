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
                          RocksDBOptions, Snapshot};
    use exonum::blockchain::{Blockchain, Transaction, Schema, Service};
    use exonum::crypto::{gen_keypair, Hash, PublicKey, SecretKey};
    use exonum::messages::{Message, RawTransaction};
    use exonum::encoding::Error as EncodingError;
    use exonum::helpers::{Height, ValidatorId};
    use exonum::node::ApiSender;

    fn create_blockchain(db: Box<Database>, services: Vec<Box<Service>>) -> Blockchain {
        let dummy_channel = mpsc::channel(1);
        let dummy_keypair = (PublicKey::zero(), SecretKey::zero());
        Blockchain::new(
            db,
            services,
            dummy_keypair.0,
            dummy_keypair.1,
            ApiSender::new(dummy_channel.0),
        )
    }

    fn execute_block(
        blockchain: &mut Blockchain,
        height: u64,
        txs: &[Hash],
        pool: &BTreeMap<Hash, Box<Transaction>>,
    ) -> Patch {

        let mut fork = blockchain.fork();
        {
            let mut schema = Schema::new(&mut fork);
            for (hash, tx) in pool {
                schema.unconfirmed_transactions_mut().put(hash, tx.raw().clone());
            }
        }
        blockchain.merge(fork.into_patch()).unwrap();
        blockchain
            .create_patch(ValidatorId::zero(), Height(height), txs)
            .unwrap()
            .1

    }

    fn execute_timestamping(db: Box<Database>, b: &mut Bencher) {

        struct Timestamping;
        impl Service for Timestamping {
            fn service_id(&self) -> u16 {
                255
            }

            fn service_name(&self) -> &'static str {
                "timestamping"
            }

            fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
                Vec::new()
            }

            fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
                Ok(Box::new(Tx::from_raw(raw)?))
            }

        }
        message! {
            struct Tx {
                const TYPE = 255;
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
        let mut blockchain = create_blockchain(db, vec![Box::new(Timestamping)]);
        for i in 0..100 {
            let (txs, pool) = prepare_txs(i, 1000);
            let patch = execute_block(&mut blockchain, i, &txs, &pool);
            blockchain.merge(patch).unwrap();
        }

        let (txs, pool) = prepare_txs(100, 1000);

        b.iter(|| execute_block(&mut blockchain, 100, &txs, &pool));
    }

    fn execute_cryptocurrency(db: Box<Database>, b: &mut Bencher) {
        struct Cryptocurrency;
        impl Service for Cryptocurrency {
            fn service_id(&self) -> u16 {
                255
            }

            fn service_name(&self) -> &'static str {
                "timestamping"
            }

            fn state_hash(&self, _: &Snapshot) -> Vec<Hash> {
                Vec::new()
            }

            fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
                Ok(Box::new(Tx::from_raw(raw)?))
            }

        }

        message! {
            struct Tx {
                const TYPE = 255;
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
                let mut index = ProofMapIndex::new("balances_txs", view);
                let from_balance = index.get(self.from()).unwrap_or(0u64);
                let to_balance = index.get(self.to()).unwrap_or(0u64);
                index.put(self.from(), from_balance - 1);
                index.put(self.to(), to_balance + 1);
            }
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


        let mut blockchain = create_blockchain(db, vec![Box::new(Cryptocurrency)]);
        let mut keys = Vec::new();

        for _ in 0..10_000 {
            keys.push(gen_keypair());
        }
        for i in 0..100 {
            let (txs, pool) = prepare_txs(i, 1000, &keys);
            let patch = execute_block(&mut blockchain, i, &txs, &pool);
            blockchain.merge(patch).unwrap();
        }

        let (txs, pool) = prepare_txs(100, 1000, &keys);

        b.iter(|| execute_block(&mut blockchain, 100, &txs, &pool));
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
