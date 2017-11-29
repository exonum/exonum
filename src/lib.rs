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

/*
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
*/

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
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
use exonum::messages::{RawTransaction, FromRaw, Message};
use exonum::encoding::serialize::json::reexport::Value;
use exonum::storage::{Fork, Snapshot, MapIndex, Entry};
use exonum::crypto::{PublicKey, Hash};
use exonum::encoding;
use exonum::helpers::fabric::{ServiceFactory, Context};
use exonum::api::Api;

// // // // // // // // // // CONSTANTS // // // // // // // // // //

const SERVICE_ID: u16 = 4;
const TX_TIME_ID: u16 = 1;
const SERVICE_NAME: &str = "exonum_time";

// // // // // // // // // // PERSISTENT DATA // // // // // // // // // //

encoding_struct! {
    struct Time {
        const SIZE = 12;

        field time:     SystemTime  [00 => 12]
    }
}

// // // // // // // // // // DATA LAYOUT // // // // // // // // // //

pub struct TimeSchema<T> {
    view: T,
}

impl<T: AsRef<Snapshot>> TimeSchema<T> {
    pub fn new(view: T) -> Self {
        TimeSchema { view }
    }

    pub fn validators_time(&self) -> MapIndex<&Snapshot, PublicKey, Time> {
        MapIndex::new(
            format!("{}.validators_time", SERVICE_NAME),
            self.view.as_ref(),
        )
    }

    pub fn time(&self) -> Entry<&Snapshot, Time> {
        Entry::new(format!("{}.time", SERVICE_NAME), self.view.as_ref())
    }
}


impl<'a> TimeSchema<&'a mut Fork> {
    pub fn validators_time_mut(&mut self) -> MapIndex<&mut Fork, PublicKey, Time> {
        MapIndex::new(format!("{}.validators_time", SERVICE_NAME), self.view)
    }

    pub fn time_mut(&mut self) -> Entry<&mut Fork, Time> {
        Entry::new(format!("{}.time", SERVICE_NAME), self.view)
    }
}

// // // // // // // // // // TRANSACTION // // // // // // // // // //

message! {
    struct TxTime {
        const TYPE = SERVICE_ID;
        const ID = TX_TIME_ID;
        const SIZE = 44;

        field time:     SystemTime  [00 => 12]
        field pub_key:  &PublicKey  [12 => 44]
    }
}

impl Transaction for TxTime {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        let validator_keys = Schema::new(&view).actual_configuration().validator_keys;

        if !validator_keys.iter().any(|&validator| {
            validator.service_key == *self.pub_key()
        })
        {
            return;
        }
        let mut schema = TimeSchema::new(view);
        match schema.validators_time().get(self.pub_key()) {
            Some(ref storage_time) if storage_time.time() >= self.time() => {
                return;
            }
            _ => {
                schema.validators_time_mut().put(
                    self.pub_key(),
                    Time::new(self.time()),
                )
            }
        }
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

        let max_byzantine_nodes = validator_keys.len() / 3;
        if validators_time.len() <= max_byzantine_nodes {
            return;
        }

        validators_time.sort_by(|a, b| b.cmp(a));

        match schema.time().get() {
            Some(ref current_time)
                if current_time.time() >= validators_time[max_byzantine_nodes] => {
                return;
            }
            _ => {
                schema.time_mut().set(Time::new(
                    validators_time[max_byzantine_nodes],
                ));
            }
        }
    }
}

// // // // // // // // // // REST API // // // // // // // // // //

#[derive(Serialize, Deserialize)]
pub struct TxResponse {
    pub tx_hash: Hash,
}

#[derive(Clone)]
struct TimeApi {
    blockchain: Blockchain,
}

impl TimeApi {
    fn get_current_time(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let schema = TimeSchema::new(&view);
        let current_time = schema.time().get();
        self.ok_response(&serde_json::to_value(current_time).unwrap())
    }

    fn get_validators_time(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let schema = TimeSchema::new(&view);
        let idx = schema.validators_time();
        let validators_time: Vec<Time> = idx.values().collect();
        if validators_time.is_empty() {
            self.not_found_response(&serde_json::to_value("Validators time database if empty")
                .unwrap())
        } else {
            self.ok_response(&serde_json::to_value(validators_time).unwrap())
        }
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

// // // // // // // // // // SERVICE DECLARATION // // // // // // // // // //

pub struct TimeService;

impl TimeService {
    pub fn new() -> TimeService {
        TimeService {}
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

    fn handle_commit(&self, context: &ServiceContext) {
        if context.validator_id().is_none() {
            return;
        }
        let (pub_key, sec_key) = (*context.public_key(), context.secret_key().clone());
        context.transaction_sender().send(Box::new(TxTime::new(
            SystemTime::now(),
            &pub_key,
            &sec_key,
        )));
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

pub struct TimeServiceFactory;

impl ServiceFactory for TimeServiceFactory {
    fn make_service(&mut self, _: &Context) -> Box<Service> {
        Box::new(TimeService::new())
    }
}
