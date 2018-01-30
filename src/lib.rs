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
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate exonum;
extern crate router;
extern crate bodyparser;
extern crate iron;

use iron::prelude::*;
use iron::Handler;
use router::Router;

use std::ops::AddAssign;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use exonum::blockchain::{Blockchain, Service, ServiceContext, Schema, Transaction, ApiContext};
use exonum::messages::{RawTransaction, Message};
use exonum::encoding::serialize::json::reexport::Value;
use exonum::storage::{Fork, Snapshot, ProofMapIndex, Entry};
use exonum::crypto::{Hash, PublicKey};
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
        /// Field that stores `SystemTime`.
        time: SystemTime,
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
    pub fn validators_time(&self) -> ProofMapIndex<&Snapshot, PublicKey, Time> {
        ProofMapIndex::new(
            format!("{}.validators_time", SERVICE_NAME),
            self.view.as_ref(),
        )
    }

    /// Returns stored `Time`.
    pub fn time(&self) -> Entry<&Snapshot, Time> {
        Entry::new(format!("{}.time", SERVICE_NAME), self.view.as_ref())
    }

    /// Returns hashes for stored tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.validators_time().root_hash(), self.time().hash()]
    }
}


impl<'a> TimeSchema<&'a mut Fork> {
    /// Mutable reference to the ['validators_time'][1] index.
    ///
    /// [1]: struct.TimeSchema.html#method.validators_time
    pub fn validators_time_mut(&mut self) -> ProofMapIndex<&mut Fork, PublicKey, Time> {
        ProofMapIndex::new(format!("{}.validators_time", SERVICE_NAME), self.view)
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
        /// Validator's time.
        time: SystemTime,
        /// Validator's public key.
        pub_key: &PublicKey,
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

/// Structure for saving validator's public key and last known local time.
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidatorTime {
    /// Validator's public key.
    pub public_key: PublicKey,
    /// Validator's time.
    pub time: Option<SystemTime>,
}

/// Shortcut to get data from storage.
impl TimeApi {
    /// Endpoint for getting value of the time that is saved in storage.
    fn get_current_time(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let schema = TimeSchema::new(&view);

        if let Some(current_time) = schema.time().get() {
            self.ok_response(&json!(Some(current_time.time())))
        } else {
            self.ok_response(&json!(None::<Box<SystemTime>>))
        }
    }

    /// Endpoint for getting time values for all validators.
    fn get_all_validators_times(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let schema = TimeSchema::new(&view);
        let idx = schema.validators_time();

        // The times of all validators for which time is known.
        let validators_time = idx.iter()
            .map(|(public_key, time)| {
                ValidatorTime {
                    public_key,
                    time: Some(time.time()),
                }
            })
            .collect::<Vec<_>>();

        self.ok_response(&serde_json::to_value(validators_time).unwrap())
    }

    /// Endpoint for getting time values for current validators.
    fn get_current_validators_times(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let validator_keys = Schema::new(&view).actual_configuration().validator_keys;
        let schema = TimeSchema::new(&view);
        let idx = schema.validators_time();

        // The times of current validators.
        // `None` if the time of the validator is unknown.
        let validators_time = validator_keys
            .iter()
            .map(|validator| {
                ValidatorTime {
                    public_key: validator.service_key,
                    time: match idx.get(&validator.service_key) {
                        Some(time) => Some(time.time()),
                        None => None,
                    },
                }
            })
            .collect::<Vec<_>>();

        self.ok_response(&serde_json::to_value(validators_time).unwrap())
    }

    fn wire_private(&self, router: &mut Router) {
        let self_ = self.clone();
        let get_current_validators_times =
            move |req: &mut Request| self_.get_current_validators_times(req);

        let self_ = self.clone();
        let get_all_validators_times = move |req: &mut Request| self_.get_all_validators_times(req);

        router.get(
            "v1/validators_times",
            get_current_validators_times,
            "get_current_validators_times",
        );

        router.get(
            "v1/validators_times/all",
            get_all_validators_times,
            "get_all_validators_times",
        );
    }
}

impl Api for TimeApi {
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let get_current_time = move |req: &mut Request| self_.get_current_time(req);
        router.get("v1/current_time", get_current_time, "get_current_time");
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

/// Mock provider for service testing.
#[derive(Debug, Clone)]
pub struct MockTimeProvider {
    /// Local time value.
    pub time: Arc<RwLock<SystemTime>>,
}

impl Default for MockTimeProvider {
    fn default() -> MockTimeProvider {
        MockTimeProvider { time: Arc::new(RwLock::new(UNIX_EPOCH)) }
    }
}

impl MockTimeProvider {
    /// Create a new `MockTimeProvider`.
    pub fn new() -> MockTimeProvider {
        MockTimeProvider::default()
    }

    /// Set the time value to `new_time`.
    pub fn set_time(&self, new_time: SystemTime) {
        let mut time = self.time.write().unwrap();
        *time = new_time;
    }

    /// Add `duration` to the value of `time`.
    pub fn add_time(&self, duration: Duration) {
        let mut time = self.time.write().unwrap();
        time.add_assign(duration);
    }
}

impl TimeProvider for MockTimeProvider {
    fn current_time(&self) -> SystemTime {
        *self.time.read().unwrap()
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

    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
        let schema = TimeSchema::new(snapshot);
        schema.state_hash()
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
