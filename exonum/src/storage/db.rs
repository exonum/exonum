use std::collections::btree_map::{BTreeMap, Range};
use std::cmp::Ordering::*;
use std::iter::Peekable;

use super::Result;

use self::NextIterValue::*;


pub type Patch = BTreeMap<Vec<u8>, Change>;
pub type Iter<'a> = Box<Iterator<'a> + 'a>;

#[derive(Debug, Clone)]
pub enum Change {
    Put(Vec<u8>),
    Delete,
}

pub struct Fork {
    snapshot: Box<Snapshot>,
    changes: Patch,
    changelog: Vec<(Vec<u8>, Option<Change>)>,
}

pub struct ForkIter<'a> {
    snapshot: Iter<'a>,
    changes: Peekable<Range<'a, Vec<u8>, Change>>,
}

#[derive(Debug, PartialEq, Eq)]
enum NextIterValue<'a> {
    Stored(&'a [u8], &'a [u8]),
    Replaced(&'a [u8], &'a [u8]),
    Inserted(&'a [u8], &'a [u8]),
    Deleted,
    MissDeleted,
    Finished
}

pub trait Database: Send + Sync + 'static {
    fn clone(&self) -> Box<Database>;
    fn snapshot(&self) -> Box<Snapshot>;
    fn fork(&self) -> Fork {
        Fork {
            snapshot: self.snapshot(),
            changes: Patch::new(),
            changelog: Vec::new(),
        }
    }
    fn merge(&mut self, patch: Patch) -> Result<()>;
}

pub trait Snapshot {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn contains(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }
    fn iter<'a>(&'a self, from: &[u8]) -> Iter<'a>;
}

pub trait Iterator<'a> {
    fn next(&mut self) -> Option<(&[u8], &[u8])>;
    fn peek(&mut self) -> Option<(&[u8], &[u8])>;
}

impl Snapshot for Fork {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self.changes.get(key) {
            Some(change) => {
                match *change {
                    Change::Put(ref v) => Some(v.clone()),
                    Change::Delete => None,
                }
            }
            None => self.snapshot.get(key),
        }
    }

    fn contains(&self, key: &[u8]) -> bool {
        match self.changes.get(key) {
            Some(change) => {
                match *change {
                    Change::Put(..) => true,
                    Change::Delete => false,
                }
            }
            None => self.snapshot.get(key).is_some(),
        }
    }

    fn iter<'a>(&'a self, from: &[u8]) -> Iter<'a> {
        use std::collections::Bound::*;
        let range = (Included(from), Unbounded);
        Box::new(ForkIter {
                     snapshot: self.snapshot.iter(from),
                     changes: self.changes.range::<[u8], _>(range).peekable(),
                 })
    }
}

impl Fork {
    pub fn checkpoint(&mut self) {
        self.changelog.clear()
    }

    pub fn rollback(&mut self) {
        for (k, c) in self.changelog.drain(..).rev() {
            match c {
                Some(change) => self.changes.insert(k, change),
                None => self.changes.remove(&k),
            };
        }
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.changelog
            .push((key.clone(), self.changes.insert(key, Change::Put(value))))
    }

    pub fn remove(&mut self, key: Vec<u8>) {
        self.changelog
            .push((key.clone(), self.changes.insert(key, Change::Delete)));
    }

    pub fn remove_by_prefix(&mut self, prefix: &[u8]) {
        let mut iter = self.snapshot.iter(prefix);

        loop {
            let k = match iter.next() {
                Some((k, _)) if k.starts_with(prefix) => k,
                _ => return
            };
            self.changes.insert(k.to_vec(), Change::Delete);
        }
    }

    pub fn into_patch(self) -> Patch {
        self.changes
    }

    pub fn merge(&mut self, patch: Patch) {
        if !self.changelog.is_empty() {
            panic!("merge into a fork is impossible because it has unfinalized changes");
        }
        self.changes.extend(patch)
    }
}

// TODO: Does we needed this AsRef / AsMut impls?

impl AsRef<Snapshot> for Snapshot {
    fn as_ref<'a>(&'a self) -> &'a (Snapshot + 'static) {
        self
    }
}

impl AsRef<Snapshot + 'static> for Fork {
    fn as_ref<'a>(&'a self) -> &'a (Snapshot + 'static) {
        &*self
    }
}

impl ::std::fmt::Debug for Fork {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "Fork(..)")
    }
}

impl<'a> NextIterValue<'a> {
    fn skip_changes(&self) -> bool {
        match *self {
            Replaced(..) | Inserted(..) | Deleted | MissDeleted => true,
            Stored(..) | Finished => false,
        }
    }

    fn skip_snapshot(&self) -> bool {
        match *self {
            Stored(..) | Replaced(..) | Deleted => true,
            Inserted(..) | MissDeleted | Finished => false,
        }
    }

    fn value(&self) -> Option<(&'a [u8], &'a [u8])> {
        match *self {
            Stored(k, v) => Some((k, v)),
            Replaced(k, v) => Some((k, v)),
            Inserted(k, v) => Some((k, v)),
            Deleted | MissDeleted | Finished => None,
        }
    }
}

impl<'a> ForkIter<'a> {
    fn step(&mut self) -> NextIterValue {
        match self.changes.peek() {
            Some(&(k, ref change)) => {
                match self.snapshot.peek() {
                    Some((key, ref value)) => {
                        match **change {
                            Change::Put(ref v) => {
                                match k[..].cmp(key) {
                                    Equal => Replaced(k, v),
                                    Less => Inserted(k, v),
                                    Greater => Stored(key, value),
                                }
                            }
                            Change::Delete => {
                                match k[..].cmp(key) {
                                    Equal => Deleted,
                                    Less => MissDeleted,
                                    Greater => Stored(key, value),
                                }
                            }
                        }
                    }
                    None => {
                        match **change {
                            Change::Put(ref v) => Inserted(k, v),
                            Change::Delete => MissDeleted,
                        }
                    }
                }
            }
            None => {
                match self.snapshot.peek() {
                    Some((key, ref value)) => Stored(key, value),
                    None => Finished,
                }
            }
        }
    }
}

impl<'a> Iterator<'a> for ForkIter<'a> {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        loop {
            let next = self.step();
            if next == Finished {
                return None;
            }
            if next.skip_changes() {
                self.changes.next();
            }
            if next.skip_snapshot() {
                self.snapshot.next();
            }
            if let Some(value) = next.value() {
                return Some(value);
            }
        }
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        loop {
            let next = self.step();
            if next == Finished {
                return None;
            }
            if let Some(value) = next.value() {
                return Some(value);
            }
            if next.skip_changes() {
                self.changes.next();
            }
            if next.skip_snapshot() {
                self.snapshot.next();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{MemoryDB, Database, Snapshot, Fork};

    #[test]
    fn fork_iter() {
        let mut db = MemoryDB::new();
        let mut fork = db.fork();

        fork.put(vec![10], vec![10]);
        fork.put(vec![20], vec![20]);
        fork.put(vec![30], vec![30]);

        db.merge(fork.into_patch()).unwrap();

        let mut fork = db.fork();

        fn assert_iter(fork: &Fork, from: u8, assumed: &[(u8, u8)]) {
            assert_eq!(fork.iter(&[from])
                           .map(|(k, v)| (k[0], v[0]))
                           .collect::<Vec<_>>(),
                       assumed);
        }

        // Stored
        assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
        assert_iter(&fork, 5, &[(10, 10), (20, 20), (30, 30)]);
        assert_iter(&fork, 10, &[(10, 10), (20, 20), (30, 30)]);
        assert_iter(&fork, 11, &[(20, 20), (30, 30)]);
        assert_iter(&fork, 31, &[]);

        // Inserted
        fork.put(vec![5], vec![5]);
        assert_iter(&fork, 0, &[(5, 5), (10, 10), (20, 20), (30, 30)]);
        fork.put(vec![25], vec![25]);
        assert_iter(&fork, 0, &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30)]);
        fork.put(vec![35], vec![35]);
        assert_iter(&fork,
                    0,
                    &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30), (35, 35)]);

        // Double inserted
        fork.put(vec![25], vec![23]);
        assert_iter(&fork,
                    0,
                    &[(5, 5), (10, 10), (20, 20), (25, 23), (30, 30), (35, 35)]);
        fork.put(vec![26], vec![26]);
        assert_iter(&fork,
                    0,
                    &[(5, 5), (10, 10), (20, 20), (25, 23), (26, 26), (30, 30), (35, 35)]);

        // Replaced
        let mut fork = db.fork();
        fork.put(vec![10], vec![11]);
        assert_iter(&fork, 0, &[(10, 11), (20, 20), (30, 30)]);
        fork.put(vec![30], vec![31]);
        assert_iter(&fork, 0, &[(10, 11), (20, 20), (30, 31)]);

        // Deleted
        let mut fork = db.fork();
        fork.remove(vec![20]);
        assert_iter(&fork, 0, &[(10, 10), (30, 30)]);
        fork.remove(vec![10]);
        assert_iter(&fork, 0, &[(30, 30)]);
        fork.put(vec![10], vec![11]);
        assert_iter(&fork, 0, &[(10, 11), (30, 30)]);
        fork.remove(vec![10]);
        assert_iter(&fork, 0, &[(30, 30)]);

        // MissDeleted
        let mut fork = db.fork();
        fork.remove(vec![5]);
        assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
        fork.remove(vec![15]);
        assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
        fork.remove(vec![35]);
        assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
    }

    #[test]
    fn changelog() {
        let db = MemoryDB::new();
        let mut fork = db.fork();

        fork.put(vec![1], vec![1]);
        fork.put(vec![2], vec![2]);
        fork.put(vec![3], vec![3]);

        assert_eq!(fork.get(&[1]), Some(vec![1]));
        assert_eq!(fork.get(&[2]), Some(vec![2]));
        assert_eq!(fork.get(&[3]), Some(vec![3]));

        fork.checkpoint();

        assert_eq!(fork.get(&[1]), Some(vec![1]));
        assert_eq!(fork.get(&[2]), Some(vec![2]));
        assert_eq!(fork.get(&[3]), Some(vec![3]));

        fork.put(vec![1], vec![10]);
        fork.put(vec![4], vec![40]);
        fork.remove(vec![2]);

        assert_eq!(fork.get(&[1]), Some(vec![10]));
        assert_eq!(fork.get(&[2]), None);
        assert_eq!(fork.get(&[3]), Some(vec![3]));
        assert_eq!(fork.get(&[4]), Some(vec![40]));

        fork.rollback();

        assert_eq!(fork.get(&[1]), Some(vec![1]));
        assert_eq!(fork.get(&[2]), Some(vec![2]));
        assert_eq!(fork.get(&[3]), Some(vec![3]));
        assert_eq!(fork.get(&[4]), None);

        fork.put(vec![4], vec![40]);
        fork.put(vec![4], vec![41]);
        fork.remove(vec![2]);
        fork.put(vec![2], vec![20]);

        assert_eq!(fork.get(&[1]), Some(vec![1]));
        assert_eq!(fork.get(&[2]), Some(vec![20]));
        assert_eq!(fork.get(&[3]), Some(vec![3]));
        assert_eq!(fork.get(&[4]), Some(vec![41]));

        fork.rollback();

        assert_eq!(fork.get(&[1]), Some(vec![1]));
        assert_eq!(fork.get(&[2]), Some(vec![2]));
        assert_eq!(fork.get(&[3]), Some(vec![3]));
        assert_eq!(fork.get(&[4]), None);

        fork.put(vec![2], vec![20]);

        fork.checkpoint();

        fork.put(vec![3], vec![30]);

        fork.rollback();

        assert_eq!(fork.get(&[1]), Some(vec![1]));
        assert_eq!(fork.get(&[2]), Some(vec![20]));
        assert_eq!(fork.get(&[3]), Some(vec![3]));
        assert_eq!(fork.get(&[4]), None);
    }
}
