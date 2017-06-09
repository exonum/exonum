use std::iter::{Iterator, Peekable};
use std::collections::btree_map::{BTreeMap, Range};
use std::cmp::Ordering::*;

use super::Result;

use self::NextIterValue::*;


pub type Patch = BTreeMap<Vec<u8>, Change>;
pub type Iter<'a> = Box<Iterator<Item=(&'a [u8], &'a [u8])> + 'a>;

#[derive(Debug)]
pub enum Change {
    Put(Vec<u8>),
    Delete,
}

pub struct Fork {
    snapshot: Box<Snapshot>,
    changes: Patch,
    changelog: Vec<(Vec<u8>, Option<Change>)>
}

pub struct ForkIter<'a> {
    snapshot: Peekable<Iter<'a>>,
    changes: Peekable<Range<'a, Vec<u8>, Change>>
}

#[derive(Debug)]
enum NextIterValue<'a> {
    Stored(&'a [u8], &'a [u8]),
    Replaced(&'a [u8], &'a [u8]),
    Inserted(&'a [u8], &'a [u8]),
    Deleted,
    MissDeleted
}

pub trait Database: Send + Sync + 'static {
    fn clone(&self) -> Box<Database>;
    fn snapshot(&self) -> Box<Snapshot>;
    fn fork(&self) -> Fork {
        Fork {
            snapshot: self.snapshot(),
            changes: Patch::new(),
            changelog: Vec::new()
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

impl Snapshot for Fork {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self.changes.get(key) {
            Some(change) => match *change {
                Change::Put(ref v) => Some(v.clone()),
                Change::Delete => None,
            },
            None => self.snapshot.get(key)
        }
    }

    fn contains(&self, key: &[u8]) -> bool {
        match self.changes.get(key) {
            Some(change) => match *change {
                Change::Put(..) => true,
                Change::Delete => false,
            },
            None => self.snapshot.get(key).is_some()
        }
    }

    fn iter<'a>(&'a self, from: &[u8]) -> Iter<'a> {
        use std::collections::Bound::*;
        let range = (Included(from), Unbounded);
        Box::new(ForkIter {
            snapshot: self.snapshot.iter(from).peekable(),
            changes: self.changes.range::<[u8], _>(range).peekable()
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
                None => self.changes.remove(&k)
            };
        }
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.changelog.push((key.clone(),
                             self.changes.insert(key, Change::Put(value))))
    }

    pub fn remove(&mut self, key: Vec<u8>) {
        self.changelog.push((key.clone(),
                             self.changes.insert(key, Change::Delete)));
    }

    pub fn remove_by_prefix(&mut self, prefix: &[u8]) {
        for (k, _) in self.snapshot.iter(prefix) {
            if !k.starts_with(prefix) {
                return
            }
            self.changes.insert(k.to_vec(), Change::Delete);
        }
    }

    pub fn into_patch(self) -> Patch {
        self.changes
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

impl<'a> NextIterValue<'a> {
    fn skip_changes(&self) -> bool {
        match *self {
            Replaced(..) | Inserted(..) | Deleted | MissDeleted => true,
            Stored(..) => false,
        }
    }

    fn skip_snapshot(&self) -> bool {
        match *self {
            Stored(..) | Replaced(..) | Deleted => true,
            Inserted(..) | MissDeleted => false
        }
    }

    fn value(&self) -> Option<(&'a [u8], &'a [u8])> {
        match *self {
            Stored(k, v) => Some((k, v)),
            Replaced(k, v) => Some((k, v)),
            Inserted(k, v) => Some((k, v)),
            Deleted | MissDeleted => None
        }
    }
}

impl<'a> Iterator for ForkIter<'a> {
    type Item = (&'a [u8], &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = match self.changes.peek() {
                Some(&(k, ref change)) => match self.snapshot.peek() {
                    Some(&(key, ref value)) => match **change {
                        Change::Put(ref v) => match k[..].cmp(key) {
                            Equal => Replaced(k, v),
                            Less => Inserted(k, v),
                            Greater => Stored(key, value)
                        },
                        Change::Delete => match k[..].cmp(key) {
                            Equal => Deleted,
                            Less => MissDeleted,
                            Greater => Stored(key, value)
                        }
                    },
                    None => match **change {
                        Change::Put(ref v) => Inserted(k, v),
                        Change::Delete => MissDeleted,
                    }
                },
                None => match self.snapshot.peek() {
                    Some(&(key, ref value)) => Stored(key, value),
                    None => return None,
                }
            };
            if next.skip_changes() {
                self.changes.next();
            }
            if next.skip_snapshot() {
                self.snapshot.next();
            }
            if let Some(value) = next.value() {
                return Some(value)
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
            assert_eq!(fork.iter(&[from]).map(|(k, v)| (k[0], v[0])).collect::<Vec<_>>(), assumed);
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
        assert_iter(&fork, 0, &[(5, 5), (10, 10), (20, 20), (25, 25), (30, 30), (35, 35)]);

        // Double inserted
        fork.put(vec![25], vec![23]);
        assert_iter(&fork, 0, &[(5, 5), (10, 10), (20, 20), (25, 23), (30, 30), (35, 35)]);
        fork.put(vec![26], vec![26]);
        assert_iter(&fork, 0, &[(5, 5), (10, 10), (20, 20), (25, 23), (26, 26), (30, 30), (35, 35)]);

        // Replaced
        let mut fork = db.fork();
        fork.put(vec![10], vec![11]);
        assert_iter(&fork, 0, &[(10, 11), (20, 20), (30, 30)]);
        fork.put(vec![30], vec![31]);
        assert_iter(&fork, 0, &[(10, 11), (20, 20), (30, 31)]);

        // Deleted
        let mut fork = db.fork();
        fork.delete(vec![20]);
        assert_iter(&fork, 0, &[(10, 10), (30, 30)]);
        fork.delete(vec![10]);
        assert_iter(&fork, 0, &[(30, 30)]);
        fork.put(vec![10], vec![11]);
        assert_iter(&fork, 0, &[(10, 11), (30, 30)]);
        fork.delete(vec![10]);
        assert_iter(&fork, 0, &[(30, 30)]);

        // MissDeleted
        let mut fork = db.fork();
        fork.delete(vec![5]);
        assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
        fork.delete(vec![15]);
        assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
        fork.delete(vec![35]);
        assert_iter(&fork, 0, &[(10, 10), (20, 20), (30, 30)]);
    }
}
