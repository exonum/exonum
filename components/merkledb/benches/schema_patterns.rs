// Copyright 2019 The Exonum Team
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

use criterion::{black_box, Bencher, Benchmark, Criterion, Throughput};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde_derive::{Deserialize, Serialize};

use std::borrow::Cow;

use exonum_crypto::Hash;
use exonum_merkledb::{
    access::{Access, AccessExt, RawAccessMut, Restore},
    impl_object_hash_for_binary_value, BinaryValue, Database, Fork, Group, KeySetIndex, Lazy,
    MapIndex, ObjectHash, ProofListIndex, ProofMapIndex, TemporaryDB,
};

const SEED: [u8; 32] = [100; 32];
const SAMPLE_SIZE: usize = 10;
const TX_COUNT: usize = 10_000;

// Parameters used in transaction processing. See `EagerSchema` definition for context.
/// Divisors used to form buckets for hot index group.
const DIVISORS: &[u64] = &[23, 31, 47];
/// Divisors used to form buckets for cold index group.
const COLD_DIVISOR: u64 = 13;
/// Chance to access `other_cold_index`.
const COLD_CHANCE: u64 = 29;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Transaction {
    value: u64,
    _payload: [u8; 32],
}

impl BinaryValue for Transaction {
    fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        bincode::deserialize(bytes.as_ref()).map_err(From::from)
    }
}

impl_object_hash_for_binary_value! { Transaction }

trait ExecuteTransaction {
    fn execute(fork: &Fork, transaction: &Transaction);
}

struct EagerSchema<T: Access> {
    // Accessed once per transaction.
    transactions: MapIndex<T::Base, Hash, Transaction>,
    // Hot index / group are accessed `DIVISORS.len()` times per transaction.
    hot_index: ProofMapIndex<T::Base, u64, Hash>,
    hot_group: Group<T, u64, ProofListIndex<T::Base, u64>>,
    // Cold index / group are accessed once per ~10 transactions.
    cold_index: ProofMapIndex<T::Base, u64, Hash>,
    cold_group: Group<T, u64, ProofListIndex<T::Base, u64>>,
    // Accessed once per ~`COLD_DIVISOR` transactions.
    other_cold_index: KeySetIndex<T::Base, u64>,
}

impl<T: Access> EagerSchema<T> {
    fn new(access: T) -> Self {
        Self {
            transactions: Restore::restore(&access, "transactions".into()).unwrap(),
            hot_index: Restore::restore(&access, "hot_index".into()).unwrap(),
            hot_group: Restore::restore(&access, "hot_group".into()).unwrap(),
            cold_index: Restore::restore(&access, "cold_index".into()).unwrap(),
            cold_group: Restore::restore(&access, "cold_group".into()).unwrap(),
            other_cold_index: Restore::restore(&access, "other_cold_index".into()).unwrap(),
        }
    }
}

impl<T: Access> EagerSchema<T>
where
    T::Base: RawAccessMut,
{
    fn execute(&mut self, transaction: &Transaction) {
        self.transactions
            .put(&transaction.object_hash(), *transaction);

        // Access hot index and group a few times.
        for &divisor in DIVISORS {
            let group_id = transaction.value % divisor;
            let mut list_in_group = self.hot_group.get(&group_id);
            list_in_group.push(transaction.value);
            self.hot_index.put(&group_id, list_in_group.object_hash());

            // Cold index / group are accessed only a fraction of the time.
            if group_id == 0 {
                let cold_group_id = transaction.value % COLD_DIVISOR;
                let mut list_in_group = self.cold_group.get(&cold_group_id);
                list_in_group.push(transaction.value);
                self.cold_index
                    .put(&cold_group_id, list_in_group.object_hash());
            }
        }

        if transaction.value % COLD_CHANCE == 0 {
            self.other_cold_index.insert(transaction.value);
        }
    }
}

impl ExecuteTransaction for EagerSchema<&'static Fork> {
    fn execute(fork: &Fork, transaction: &Transaction) {
        let mut schema = EagerSchema::new(fork);
        schema.execute(transaction);
    }
}

struct LazySchema<T: Access> {
    transactions: MapIndex<T::Base, Hash, Transaction>,
    hot_index: ProofMapIndex<T::Base, u64, Hash>,
    hot_group: Group<T, u64, ProofListIndex<T::Base, u64>>,
    cold_index: Lazy<T, ProofMapIndex<T::Base, u64, Hash>>,
    // groups are already lazy
    cold_group: Group<T, u64, ProofListIndex<T::Base, u64>>,
    other_cold_index: Lazy<T, KeySetIndex<T::Base, u64>>,
}

impl<T: Access> LazySchema<T> {
    fn new(access: T) -> Self {
        Self {
            transactions: Restore::restore(&access, "transactions".into()).unwrap(),
            hot_index: Restore::restore(&access, "hot_index".into()).unwrap(),
            hot_group: Restore::restore(&access, "hot_group".into()).unwrap(),
            cold_index: Restore::restore(&access, "cold_index".into()).unwrap(),
            cold_group: Restore::restore(&access, "cold_group".into()).unwrap(),
            other_cold_index: Restore::restore(&access, "other_cold_index".into()).unwrap(),
        }
    }
}

impl<T: Access> LazySchema<T>
where
    T::Base: RawAccessMut,
{
    fn execute(&mut self, transaction: &Transaction) {
        self.transactions
            .put(&transaction.object_hash(), *transaction);

        // Access hot index and group a few times.
        for &divisor in DIVISORS {
            let group_id = transaction.value % divisor;
            let mut list_in_group = self.hot_group.get(&group_id);
            list_in_group.push(transaction.value);
            self.hot_index.put(&group_id, list_in_group.object_hash());

            // Cold index / group are accessed only a fraction of the time.
            if group_id == 0 {
                let cold_group_id = transaction.value % COLD_DIVISOR;
                let mut list_in_group = self.cold_group.get(&cold_group_id);
                list_in_group.push(transaction.value);
                self.cold_index
                    .get()
                    .put(&cold_group_id, list_in_group.object_hash());
            }
        }

        if transaction.value % COLD_CHANCE == 0 {
            self.other_cold_index.get().insert(transaction.value);
        }
    }
}

impl ExecuteTransaction for LazySchema<&'static Fork> {
    fn execute(fork: &Fork, transaction: &Transaction) {
        let mut schema = LazySchema::new(fork);
        schema.execute(transaction);
    }
}

struct WrapperSchema<T>(T);

impl<T: Access> WrapperSchema<T> {
    fn new(access: T) -> Self {
        Self(access)
    }

    fn transactions(&self) -> MapIndex<T::Base, Hash, Transaction> {
        self.0.get_map("transactions")
    }

    fn hot_index(&self) -> ProofMapIndex<T::Base, u64, Hash> {
        self.0.get_proof_map("hot_index")
    }

    fn hot_group(&self, group_id: u64) -> ProofListIndex<T::Base, u64> {
        self.0.get_proof_list(("hot_group", &group_id))
    }

    fn cold_index(&self) -> ProofMapIndex<T::Base, u64, Hash> {
        self.0.get_proof_map("cold_index")
    }

    fn cold_group(&self, group_id: u64) -> ProofListIndex<T::Base, u64> {
        self.0.get_proof_list(("cold_group", &group_id))
    }

    fn other_cold_index(&self) -> KeySetIndex<T::Base, u64> {
        self.0.get_key_set("other_cold_index")
    }
}

impl<T: Access> WrapperSchema<T>
where
    T::Base: RawAccessMut,
{
    fn execute(&self, transaction: &Transaction) {
        self.transactions()
            .put(&transaction.object_hash(), *transaction);

        // Access hot index and group a few times.
        let mut hot_index = self.hot_index();

        for &divisor in DIVISORS {
            let group_id = transaction.value % divisor;
            let mut list_in_group = self.hot_group(group_id);
            list_in_group.push(transaction.value);
            hot_index.put(&group_id, list_in_group.object_hash());

            // Cold index / group are accessed only a fraction of the time.
            if group_id == 0 {
                let cold_group_id = transaction.value % COLD_DIVISOR;
                let mut list_in_group = self.cold_group(cold_group_id);
                list_in_group.push(transaction.value);
                self.cold_index()
                    .put(&cold_group_id, list_in_group.object_hash());
            }
        }

        if transaction.value % COLD_CHANCE == 0 {
            self.other_cold_index().insert(transaction.value);
        }
    }
}

impl ExecuteTransaction for WrapperSchema<()> {
    fn execute(fork: &Fork, transaction: &Transaction) {
        let schema = WrapperSchema::new(fork);
        schema.execute(transaction);
    }
}

fn gen_random_transactions(count: usize) -> Vec<Transaction> {
    let mut rng = StdRng::from_seed(SEED);
    (0..count)
        .map(|_| Transaction {
            value: rng.gen(),
            _payload: rng.gen(),
        })
        .collect()
}

fn bench<T: ExecuteTransaction>(bencher: &mut Bencher) {
    let transactions = gen_random_transactions(TX_COUNT);
    bencher.iter_with_setup(TemporaryDB::new, |db| {
        let fork = db.fork();
        for transaction in &transactions {
            T::execute(black_box(&fork), transaction);
            // ^-- prevent compiler from moving schema initialization from outside the loop.
        }
    })
}

pub fn bench_schema_patterns(c: &mut Criterion) {
    exonum_crypto::init();

    c.bench(
        "schema_patterns",
        Benchmark::new("eager", bench::<EagerSchema<&'static Fork>>)
            .with_function("lazy", bench::<LazySchema<&'static Fork>>)
            .with_function("wrapper", bench::<WrapperSchema<()>>)
            .throughput(Throughput::Elements(TX_COUNT as u64))
            .sample_size(SAMPLE_SIZE),
    );
}
