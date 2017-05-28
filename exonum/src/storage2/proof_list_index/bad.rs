use num::{Integer, range, ToPrimitive, pow};

use std::marker::PhantomData;
use std::cell::Cell;

use crypto::Hash;
use super::{BaseTable, View, List, Error, VoidKey, StorageValue};
use self::proofnode::Proofnode;


mod proofnode;

impl<'a, V: StorageValue> MerkleTable<'a, V> {
}
