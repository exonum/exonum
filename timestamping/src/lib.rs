#![feature(question_mark)]

#[macro_use(message)]
extern crate exonum;

use std::borrow::{Borrow, BorrowMut};

use exonum::crypto::PublicKey;
use exonum::storage::{Blockchain, Database};

pub const TIMESTAMPING_TRANSACTION_MESSAGE_ID : u16 = 128;

message! {
    TimestampTx {
        const ID = TIMESTAMPING_TRANSACTION_MESSAGE_ID;
        const SIZE = 40;

        pub_key:        &PublicKey  [00 => 32]
        data:           &[u8]       [32 => 40]
    }
}

pub struct TimestampingBlockchain<D: Database> {
    pub db: D
}

impl<D: Database> Borrow<D> for TimestampingBlockchain<D> {
    fn borrow(&self) -> &D {
        &self.db
    }
}

impl<D: Database> BorrowMut<D> for TimestampingBlockchain<D> {
    fn borrow_mut(&mut self) -> &mut D {
        &mut self.db
    }
}


impl<D> Blockchain for TimestampingBlockchain<D> where D: Database {
    type Database = D;
    type Transaction = TimestampTx;
}
