// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.


//! Some logging related stuff
use slog::{OwnedKVList, Record, Drain, Logger, SendSyncUnwindSafeDrain,
           SendSyncRefUnwindSafeDrain, Never};
use slog_async::Async;
use slog_scope::{GlobalLoggerGuard, set_global_logger};
use std::iter::FromIterator;
use std::cell::Cell;
use std::sync::Arc;

use messages::{Connect, Propose, Prevote, Precommit, PrevotesRequest, BlockResponse, Status,
               ProposeRequest, TransactionsRequest, PeersRequest, BlockRequest, ConsensusMessage};

pub use self::config::LoggerConfig;

thread_local!(static GLOBAL_LOGGER: Cell<Option<GlobalLoggerGuard>> = Cell::new(None););
mod config;
mod builder;
// TODO: replace before merge
// Stub for future replacement
pub(crate) type ExonumLogger = Arc<SendSyncRefUnwindSafeDrain<Ok = (), Err = Never>>;
/// Performs the logger initialization.
pub(crate) fn init_logger(logger_config: LoggerConfig) -> ExonumLogger {
    let async = Async::new(logger_config.into_multi_logger()).build().fuse();
    let ret = Arc::new(async);
    let logger_cloned = Arc::clone(&ret);

    GLOBAL_LOGGER.with(|v| {
        v.set(Some(set_global_logger(
            Logger::root_typed(logger_cloned, o!("module" => "global")),
        )))
    });
    ret
}

pub(crate) struct MultipleDrain {
    drains: Vec<Box<Drain<Ok = (), Err = Never> + Send>>,
}

impl FromIterator<Box<Drain<Ok = (), Err = Never> + Send>> for MultipleDrain {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Box<Drain<Ok = (), Err = Never> + Send>>,
    {
        MultipleDrain { drains: iter.into_iter().collect() }
    }
}

impl Drain for MultipleDrain {
    type Ok = ();
    type Err = Never;
    fn log(&self, record: &Record, logger_values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        for drain in &self.drains {
            drop(drain.log(record, logger_values));
        }
        Ok(())
    }
}


/// `ExtContextLogger` is a trait, used to compatible extend root logging context.
/// This trait is useful when you need to introduce some context
/// logging into types that not belong to you
pub(crate) trait ExtContextLogger<L>
where L: SendSyncUnwindSafeDrain<Ok = (), Err = Never> + Clone + 'static
{
    type Logger: Drain;
    fn logger(&self, root_logger: &L) -> Self::Logger;
}

use slog::SingleKV;

impl<L> ExtContextLogger<L> for Connect
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "connect"),
                SingleKV("msg_public_key", format!("{:?}", self.pub_key())),
                SingleKV("msg_address", format!("{}", self.addr()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for Propose
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "propose"),
                SingleKV("msg_validator", format!("{}", self.validator())),
                SingleKV("msg_height", format!("{}", self.height())),
                SingleKV("msg_round", format!("{}", self.round()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for Prevote
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "prevote"),
                SingleKV("msg_validator", format!("{}", self.validator())),
                SingleKV("msg_height", format!("{}", self.height())),
                SingleKV("msg_round", format!("{}", self.round())),
                SingleKV("msg_locked_round", format!("{}", self.locked_round()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for Precommit
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "precommit"),
                SingleKV("msg_validator_id", format!("{}", self.validator())),
                SingleKV("msg_height", format!("{}", self.height())),
                SingleKV("msg_round", format!("{}", self.round()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for Status
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "status"),
                SingleKV("msg_public_key", format!("{:?}", self.from())),
                SingleKV("msg_height", format!("{}", self.height()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for ProposeRequest
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "propose_request"),
                SingleKV("msg_public_key", format!("{:?}", self.from())),
                SingleKV("msg_height", format!("{}", self.height()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for TransactionsRequest
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "transaction_request"),
                SingleKV("msg_public_key", format!("{:?}", self.from())),
                SingleKV("msg_to", format!("{:?}", self.to()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for PrevotesRequest
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "transaction_request"),
                SingleKV("msg_public_key", format!("{:?}", self.from())),
                SingleKV("msg_to", format!("{:?}", self.to())),
                SingleKV("msg_height", format!("{}", self.height())),
                SingleKV("msg_round", format!("{}", self.round()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for PeersRequest
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {

        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", format!("{:?}", "peers_request")),
                SingleKV("msg_public_key", format!("{:?}", self.from())),
                SingleKV("msg_to", format!("{:?}", self.to()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for BlockRequest
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "block_request"),
                SingleKV("msg_public_key", format!("{:?}", self.from())),
                SingleKV("msg_to", format!("{:?}", self.to())),
                SingleKV("msg_height", format!("{}", self.height()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for BlockResponse
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "block_response"),
                SingleKV("msg_public_key", format!("{:?}", self.from())),
                SingleKV("msg_to", format!("{:?}", self.to())),
                SingleKV("msg_height", format!("{}", self.block().height()))
            ),
        )
    }
}

impl<L> ExtContextLogger<L> for ConsensusMessage
where
    L: SendSyncUnwindSafeDrain<Ok = (), Err = Never>
        + Clone
        + 'static,
{
    type Logger = Logger<L>;
    fn logger(&self, root_logger: &L) -> Self::Logger {
        Logger::root_typed(
            root_logger.clone(),
            o!(
                SingleKV("msg_type", "consensus_message"),
                SingleKV("msg_validator", format!("{}", self.validator())),
                SingleKV("msg_round", format!("{}", self.round())),
                SingleKV("msg_height", format!("{}", self.height()))
            ),
        )
    }
}
