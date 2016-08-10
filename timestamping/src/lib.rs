#![feature(question_mark)]

#[macro_use(message)]
extern crate exonum;

use exonum::crypto::PublicKey;
use exonum::storage::{Blockchain, LevelDB, Fork};

pub const TIMESTAMPING_TRANSACTION_MESSAGE_ID : u16 = 128;

message! {
    TimestampTx {
        const ID = TIMESTAMPING_TRANSACTION_MESSAGE_ID;
        const SIZE = 40;

        pub_key:        &PublicKey  [00 => 32]
        data:           &[u8]       [32 => 40]
    }
}

struct TimestampingBlockchain {
    db: LevelDB
}

impl Blockchain for TimestampingBlockchain {
    type Database = LevelDB;
    type Transaction = TimestampTx;

    fn db(&self) -> &Self::Database {
        &self.db
    }

    fn db_mut(&mut self) -> &mut Self::Database {
        &mut self.db
    }
}
