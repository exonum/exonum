use criterion::{black_box, Bencher, Criterion};
use exonum_merkledb::{
    Database, IndexAccess, LazyListIndex, ListIndex, ObjectAccess, ObjectHash, ProofListIndex,
    RefMut, TemporaryDB,
};
use rand::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

const SEED: [u8; 16] = [100; 16];
const ITEM_COUNT: u16 = 1000;

fn bench_fn<T, F>(b: &mut Bencher, index_access: T, benchmark: F)
where
    T: IndexAccess,
    F: Fn(T),
{
    b.iter(|| benchmark(index_access.clone()))
}

fn bench_default_list<T: IndexAccess>(index_access: T) {
    let mut rng = XorShiftRng::from_seed(SEED);
    let mut index: ProofListIndex<_, u32> = ProofListIndex::new("index", index_access.clone());
    for _ in 0..ITEM_COUNT {
        index.push(rng.next_u32());
    }
    let hash = index.object_hash();
    black_box(hash);
}

fn bench_lazy_list<T: ObjectAccess>(index_access: T) {
    let mut rng = XorShiftRng::from_seed(SEED);
    let mut index: LazyListIndex<_, u32> = LazyListIndex::new("index2", index_access.clone());
    for _ in 0..ITEM_COUNT {
        index.push(rng.next_u32());
    }
    index.update_hashes();
    let hash = index.object_hash();
    black_box(hash);
}

pub fn bench_lazy_hash(c: &mut Criterion) {
    c.bench_function("lazy/index/default", move |b| {
        let db = TemporaryDB::new();
        let fork = db.fork();
        bench_fn(b, &fork, |fork| bench_default_list(fork));
    });

    c.bench_function("lazy/index/lazy_object_hash", move |b| {
        let db = TemporaryDB::new();
        let fork = db.fork();
        bench_fn(b, &fork, |fork| bench_lazy_list(fork));
    });
}
