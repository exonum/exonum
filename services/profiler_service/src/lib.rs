#[macro_use(message)]
extern crate exonum;
#[macro_use]
extern crate log;

extern crate profiler;

use std::env;


use exonum::messages::{Message, RawTransaction, Error as MessageError};
use exonum::crypto::{Hash, hash};
use exonum::storage::{Error, View as StorageView};
use exonum::blockchain::{Service, Transaction};
use exonum::node::State;

pub const PROFILER_SERVICE: u16 = 7001;
pub const PROFILER_TRANSACTION_MESSAGE_ID: u16 = 7002;


const PROFILE_ENV_VARIABLE_NAME: &'static str = "PROFILE_FILENAME";

fn flame_dump() {
    use profiler;
    use std::fs::File;

    if File::create(env::var(PROFILE_ENV_VARIABLE_NAME)
                          .unwrap_or("exonum-flame-graph.html".to_string()))
             .and_then(|ref mut  file| profiler::dump_html(file) ).is_err() {
        warn!("FLAME_GRAPH, cant dump html!");
    }

   
}

message! {
    ProfilerTx {
        const TYPE = PROFILER_SERVICE;
        const ID = PROFILER_TRANSACTION_MESSAGE_ID;
        const SIZE = 01;

        dump:      bool         [00 => 01]
    }
}

pub struct ProfilerService {}

impl ProfilerService {
    pub fn new() -> ProfilerService {
        ProfilerService {}
    }
}

impl Transaction for ProfilerTx {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, _: &StorageView) -> Result<(), Error> {
        if self.dump() && cfg!(feature="flame_profile") {
            flame_dump();
        }
        Ok(())
    }

    fn raw(&self) -> &RawTransaction {
        Message::raw(self)
    }

    fn clone_box(&self) -> Box<Transaction> {
        Box::new(self.clone())
    }

    fn hash(&self) -> Hash {
        Message::hash(self)
    }
}

impl Service for ProfilerService {
    fn service_id(&self) -> u16 {
        PROFILER_SERVICE
    }

    fn handle_genesis_block(&self, _: &StorageView) -> Result<(), Error> {
        Ok(())
    }

    fn state_hash(&self, _: &StorageView) -> Result<Hash, Error> {
        Ok(hash(&[]))
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        if raw.message_type() != PROFILER_TRANSACTION_MESSAGE_ID {
            return Err(MessageError::IncorrectMessageType { message_type: raw.message_type() });
        }

        ProfilerTx::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn handle_commit(&self,
                     _: &StorageView,
                     _: &mut State)
                     -> Result<Vec<Box<Transaction>>, Error> {
        Ok(Vec::new())
    }
}