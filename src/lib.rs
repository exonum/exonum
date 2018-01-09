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

//! The time oracle service for Exonum.

#![deny(missing_debug_implementations, missing_docs)]

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate exonum;
extern crate router;
extern crate bodyparser;
extern crate iron;

use iron::prelude::*;
use iron::Handler;
use router::Router;

use std::time::SystemTime;

use exonum::blockchain::{Blockchain, Service, ServiceContext, Schema, Transaction, ApiContext};
use exonum::messages::{RawTransaction, Message};
use exonum::encoding::serialize::json::reexport::Value;
use exonum::storage::{Fork, Snapshot, MapIndex, Entry};
use exonum::crypto::PublicKey;
use exonum::encoding;
use exonum::helpers::fabric::{ServiceFactory, Context};
use exonum::api::Api;

/// Time service id.
const SERVICE_ID: u16 = 4;
/// `TxTime` transaction id.
const TX_TIME_ID: u16 = 1;
/// Time service name.
const SERVICE_NAME: &str = "exonum_time";

encoding_struct! {
    /// Time information.
    struct Time {
        const SIZE = 12;

        /// Field that stores `SystemTime`.
        field time:     SystemTime  [00 => 12]
    }
}

/// `Exonum-time` service database schema.
#[derive(Debug)]
pub struct TimeSchema<T> {
    view: T,
}

impl<T: AsRef<Snapshot>> TimeSchema<T> {
    /// Constructs schema for the given `snapshot`.
    pub fn new(view: T) -> Self {
        TimeSchema { view }
    }

    /// Returns the table that stores `Time` struct for every validator.
    pub fn validators_time(&self) -> MapIndex<&Snapshot, PublicKey, Time> {
        MapIndex::new(
            format!("{}.validators_time", SERVICE_NAME),
            self.view.as_ref(),
        )
    }

    /// Returns stored `Time`.
    pub fn time(&self) -> Entry<&Snapshot, Time> {
        Entry::new(format!("{}.time", SERVICE_NAME), self.view.as_ref())
    }
}


impl<'a> TimeSchema<&'a mut Fork> {
    /// Mutable reference to the ['validators_time'][1] index.
    ///
    /// [1]: struct.TimeSchema.html#method.validators_time
    pub fn validators_time_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Time> {
        MapIndex::new(format!("{}.validators_time", SERVICE_NAME), self.view)
    }

    /// Mutable reference to the ['time'][1] index.
    ///
    /// [1]: struct.TimeSchema.html#method.time
    pub fn time_mut(&mut self) -> Entry<&mut Fork, Time> {
        Entry::new(format!("{}.time", SERVICE_NAME), self.view)
    }
}

message! {
    /// Transaction that is sent by the validator after the commit of the block.
    struct TxTime {
        const TYPE = SERVICE_ID;
        const ID = TX_TIME_ID;
        const SIZE = 44;
        /// Validator's time.
        field time:     SystemTime  [00 => 12]
        /// Validator's public key.
        field pub_key:  &PublicKey  [12 => 44]
    }
}

impl Transaction for TxTime {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        let validator_keys = Schema::new(&view).actual_configuration().validator_keys;

        // The transaction must be signed by the validator.
        let signed = validator_keys.iter().any(|&validator| {
            validator.service_key == *self.pub_key()
        });
        if !signed {
            return;
        }

        let mut schema = TimeSchema::new(view);
        match schema.validators_time().get(self.pub_key()) {
            // The validator time in the storage should be less than in the transaction.
            Some(ref storage_time) if storage_time.time() >= self.time() => {
                return;
            }
            // Write the time for the validator.
            _ => {
                schema.validators_time_mut().put(
                    self.pub_key(),
                    Time::new(self.time()),
                )
            }
        }

        // Find all known times for the validators.
        let mut validators_time: Vec<SystemTime>;
        {
            let idx = schema.validators_time();
            validators_time = idx.iter()
                .filter_map(|pair| {
                    validator_keys
                        .iter()
                        .find(|validator| validator.service_key == pair.0)
                        .map(|_| pair.1.time())
                })
                .collect();
        }

        // The largest number of Byzantine nodes.
        let max_byzantine_nodes = (validator_keys.len() - 1) / 3;
        if validators_time.len() <= 2 * max_byzantine_nodes {
            return;
        }
        // Ordering time from highest to lowest.
        validators_time.sort_by(|a, b| b.cmp(a));

        match schema.time().get() {
            // Selected time should be longer than the time in the storage.
            Some(ref current_time)
                if current_time.time() >= validators_time[max_byzantine_nodes] => {
                return;
            }
            _ => {
                // Change the time in the storage.
                schema.time_mut().set(Time::new(
                    validators_time[max_byzantine_nodes],
                ));
            }
        }
    }
}

/// Implements the node API.
#[derive(Clone)]
struct TimeApi {
    blockchain: Blockchain,
}

encoding_struct! {
    /// Structure for saving validator's public key and last known local time.
    struct ValidatorTime {
        const SIZE = 44;

        /// Validator's public key.
        field public_key:     &PublicKey      [00 => 32]
        /// Validator's time.
        field time:           SystemTime      [32 => 44]
    }
}

/// Shortcut to get data from storage.
impl TimeApi {
    /// Endpoint for getting value of the time that is saved in storage.
    fn get_current_time(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let schema = TimeSchema::new(&view);
        let current_time = schema.time().get();
        self.ok_response(&serde_json::to_value(current_time).unwrap())
    }

    /// Endpoint for getting time values for all validators.
    fn get_validators_time(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let schema = TimeSchema::new(&view);
        let idx = schema.validators_time();

        let validators_time: Vec<_> = idx.iter()
            .map(|(public_key, time)| {
                ValidatorTime::new(&public_key, time.time())
            })
            .collect();

        self.ok_response(&serde_json::to_value(validators_time).unwrap())
    }

    fn wire_private(&self, router: &mut Router) {
        let self_ = self.clone();
        let get_validators_time = move |req: &mut Request| self_.get_validators_time(req);
        router.get(
            "/validators_time",
            get_validators_time,
            "get_validators_time",
        );
    }
}

impl Api for TimeApi {
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let get_current_time = move |req: &mut Request| self_.get_current_time(req);
        router.get("/current_time", get_current_time, "get_current_time");
    }
}

/// A helper trait that provides the node with a current time.
pub trait TimeProvider: Send + Sync + ::std::fmt::Debug {
    /// Returns the current time.
    fn current_time(&self) -> SystemTime;
}

#[derive(Debug)]
struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn current_time(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Define the service.
#[derive(Debug)]
pub struct TimeService {
    /// Current time.
    time: Box<TimeProvider>,
}

impl Default for TimeService {
    fn default() -> TimeService {
        TimeService { time: Box::new(SystemTimeProvider) as Box<TimeProvider> }
    }
}

impl TimeService {
    /// Create a new `TimeService`.
    pub fn new() -> TimeService {
        TimeService::default()
    }

    /// Create a new `TimeService` with time provider `T`.
    pub fn with_provider<T: Into<Box<TimeProvider>>>(time_provider: T) -> TimeService {
        TimeService { time: time_provider.into() }
    }
}

impl Service for TimeService {
    fn service_name(&self) -> &'static str {
        SERVICE_NAME
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        match raw.message_type() {
            TX_TIME_ID => Ok(Box::new(TxTime::from_raw(raw)?)),
            _ => {
                let error =
                    encoding::Error::IncorrectMessageType { message_type: raw.message_type() };
                Err(error)
            }
        }
    }

    fn initialize(&self, _fork: &mut Fork) -> Value {
        Value::Null
    }

    /// Creates transaction after commit of the block.
    fn handle_commit(&self, context: &ServiceContext) {
        // The transaction must be created by the validator.
        if context.validator_id().is_none() {
            return;
        }
        let (pub_key, sec_key) = (*context.public_key(), context.secret_key().clone());
        context
            .transaction_sender()
            .send(Box::new(
                TxTime::new(self.time.current_time(), &pub_key, &sec_key),
            ))
            .unwrap();
    }

    fn private_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = TimeApi { blockchain: ctx.blockchain().clone() };
        api.wire_private(&mut router);
        Some(Box::new(router))
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = TimeApi { blockchain: ctx.blockchain().clone() };
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

/// A time service creator for the `NodeBuilder`.
#[derive(Debug)]
pub struct TimeServiceFactory;

impl ServiceFactory for TimeServiceFactory {
    fn make_service(&mut self, _: &Context) -> Box<Service> {
        Box::new(TimeService::new())
    }
}
