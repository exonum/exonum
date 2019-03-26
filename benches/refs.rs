use criterion::{black_box, Bencher, Criterion};
use exonum_merkledb::{Database, IndexAccess, ListIndex, ObjectAccess, RefMut, TemporaryDB};
use rand::{RngCore, SeedableRng};
use rand_xorshift::XorShiftRng;

const SEED: [u8; 16] = [100; 16];
const ITEM_COUNT: u16 = 1000;

fn bench_index_create(b: &mut Bencher) {
    b.iter_with_setup(
        || {
            let db = TemporaryDB::new();
            db.fork()
        },
        |fork| {
            bench_with_index_access(&fork);
        },
    );
}

fn bench_index_create_ref(b: &mut Bencher) {
    b.iter_with_setup(
        || {
            let db = TemporaryDB::new();
            db.fork()
        },
        |fork| {
            bench_with_object_access(&fork);
        },
    );
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
        let index: RefMut<ListIndex<_, u32>> =
            object_access.get_or_create_object(("index", &rng.next_u32()));
        black_box(index);
    }
}

pub fn bench_refs(c: &mut Criterion) {
    c.bench_function("refs/index/create/default", bench_index_create);
    c.bench_function("refs/index/create/get_or_create", bench_index_create_ref);
}
