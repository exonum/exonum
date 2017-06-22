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

// FIXME: make &mut Fork "unwind safe"
pub struct Fork {
    snapshot: Box<Snapshot>,
    changes: Patch,
    changelog: Vec<(Vec<u8>, Option<Change>)>,
    logged: bool,
}

pub struct ForkIter<'a> {
    snapshot: Iter<'a>,
    changes: Peekable<Range<'a, Vec<u8>, Change>>,
}

#[derive(Debug, PartialEq, Eq)]
enum NextIterValue {
    Stored,
    Replaced,
    Inserted,
    Deleted,
    MissDeleted,
    Finished,
}

pub trait Database: Send + Sync + 'static {
    fn clone(&self) -> Box<Database>;
    fn snapshot(&self) -> Box<Snapshot>;
    fn fork(&self) -> Fork {
        Fork {
            snapshot: self.snapshot(),
            changes: Patch::new(),
            changelog: Vec::new(),
            logged: false,
        }
    }
    fn merge(&mut self, patch: Patch) -> Result<()>;
}

pub trait Snapshot: 'static {
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
        if self.logged {
            panic!("call checkpoint before rollback or commit");
        }
        self.logged = true;
    }

    pub fn commit(&mut self) {
        if !self.logged {
            panic!("call commit before checkpoint");
        }
        self.changelog.clear();
        self.logged = false;
    }

    pub fn rollback(&mut self) {
        if !self.logged {
            panic!("call rollback before checkpoint");
        }
        for (k, c) in self.changelog.drain(..).rev() {
            match c {
                Some(change) => self.changes.insert(k, change),
                None => self.changes.remove(&k),
            };
        }
        self.logged = false;
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        if self.logged {
            self.changelog
                .push((key.clone(), self.changes.insert(key, Change::Put(value))));
        } else {
            self.changes.insert(key, Change::Put(value));
        }
    }

    pub fn remove(&mut self, key: Vec<u8>) {
        if self.logged {
            self.changelog
                .push((key.clone(), self.changes.insert(key, Change::Delete)));
        } else {
            self.changes.insert(key, Change::Delete);
        }
    }

    pub fn remove_by_prefix(&mut self, prefix: &[u8]) {
        let mut iter = self.snapshot.iter(prefix);
        while let Some((k, ..)) = iter.next() {
            if !k.starts_with(prefix) {
                return;
            }
            let change = self.changes.insert(k.to_vec(), Change::Delete);
            if self.logged {
                self.changelog.push((k.to_vec(), change));
            }
        }
    }

    pub fn into_patch(self) -> Patch {
        self.changes
    }

    pub fn merge(&mut self, patch: Patch) {
        if self.logged {
            panic!("call merge before commit or rollback");
        }
        self.changes.extend(patch)
    }
}

impl AsRef<Snapshot> for Snapshot + 'static {
    fn as_ref(&self) -> &Snapshot {
        self
    }
}

impl AsRef<Snapshot> for Fork {
    fn as_ref(&self) -> &Snapshot {
        &*self
    }
}

impl ::std::fmt::Debug for Fork {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(f, "Fork(..)")
    }
}

impl<'a> ForkIter<'a> {
    fn step(&mut self) -> NextIterValue {
        match self.changes.peek() {
            Some(&(k, change)) => {
                match self.snapshot.peek() {
                    Some((key, ..)) => {
                        match *change {
                            Change::Put(..) => {
                                match k[..].cmp(key) {
                                    Equal => Replaced,
                                    Less => Inserted,
                                    Greater => Stored,
                                }
                            }
                            Change::Delete => {
                                match k[..].cmp(key) {
                                    Equal => Deleted,
                                    Less => MissDeleted,
                                    Greater => Stored,
                                }
                            }
                        }
                    }
                    None => {
                        match *change {
                            Change::Put(..) => Inserted,
                            Change::Delete => MissDeleted,
                        }
                    }
                }
            }
            None => {
                match self.snapshot.peek() {
                    Some(..) => Stored,
                    None => Finished,
                }
            }
        }
    }
}

impl<'a> Iterator<'a> for ForkIter<'a> {
    fn next(&mut self) -> Option<(&[u8], &[u8])> {
        loop {
            match self.step() {
                Stored => return self.snapshot.next(),
                Replaced => {
                    self.snapshot.next();
                    return self.changes
                               .next()
                               .map(|(key, change)| {
                                        (key.as_slice(),
                                         match *change {
                                             Change::Put(ref value) => value.as_slice(),
                                             Change::Delete => unreachable!(),
                                         })
                                    });
                }
                Inserted => {
                    return self.changes
                               .next()
                               .map(|(key, change)| {
                                        (key.as_slice(),
                                         match *change {
                                             Change::Put(ref value) => value.as_slice(),
                                             Change::Delete => unreachable!(),
                                         })
                                    })
                }
                Deleted => {
                    self.changes.next();
                    self.snapshot.next();
                }
                MissDeleted => {
                    self.changes.next();
                }
                Finished => return None,
            }
        }
    }

    fn peek(&mut self) -> Option<(&[u8], &[u8])> {
        loop {
            match self.step() {
                Stored | Replaced => return self.snapshot.peek(),
                Inserted => {
                    return self.changes
                               .peek()
                               .map(|&(key, change)| {
                                        (key.as_slice(),
                                         match *change {
                                             Change::Put(ref value) => value.as_slice(),
                                             Change::Delete => unreachable!(),
                                         })
                                    })
                }
                Deleted => {
                    self.changes.next();
                    self.snapshot.next();
                }
                MissDeleted => {
                    self.changes.next();
                }
                Finished => return None,
            }
        }
    }
}
