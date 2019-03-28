use criterion::{black_box, Bencher, Criterion};
use exonum_merkledb::{Database, IndexAccess, ListIndex, ObjectAccess, RefMut, TemporaryDB};
use rand::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

const SEED: [u8; 16] = [100; 16];
const ITEM_COUNT: u16 = 10000;

fn bench_fn<T, F>(b: &mut Bencher, index_access: T, benchmark: F)
where
    T: IndexAccess,
    F: Fn(T),
{
    b.iter(|| benchmark(index_access))
}

fn bench_with_index_access<T: IndexAccess>(index_access: T) {
    for _ in 0..ITEM_COUNT {
        let mut rng = XorShiftRng::from_seed(SEED);
        let index: ListIndex<_, u32> =
            ListIndex::new_in_family("index", &rng.next_u32(), index_access);
        black_box(index);
    }
}

fn bench_with_object_access<T: ObjectAccess>(object_access: T) {
    for _ in 0..ITEM_COUNT {
        let mut rng = XorShiftRng::from_seed(SEED);
        let index: RefMut<ListIndex<_, u32>> = object_access.get_object(("index", &rng.next_u32()));
        black_box(index);
    }
}

pub fn bench_refs(c: &mut Criterion) {
    c.bench_function("refs/index/create/default", move |b| {
        let db = TemporaryDB::new();
        let fork = db.fork();
        bench_fn(b, &fork, |fork| bench_with_index_access(fork));
    });

    c.bench_function("refs/index/create/get_or_create", move |b| {
        let db = TemporaryDB::new();
        let fork = db.fork();
        bench_fn(b, &fork, |fork| bench_with_object_access(fork));
    });
}
