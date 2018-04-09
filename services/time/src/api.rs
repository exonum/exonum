use std::fmt;

use exonum::api::Api;
use exonum::blockchain::{Blockchain, Schema};
use exonum::crypto::PublicKey;
use exonum::encoding::serialize::json::reexport as serde_json;

use chrono::{DateTime, Utc};
use iron::prelude::*;
use router::Router;

use super::TimeSchema;

/// Implements the node API.
#[derive(Clone)]
pub struct TimeApi {
    /// Exonum blockchain.
    pub blockchain: Blockchain,
}

/// Structure for saving public key of the validator and last known local time.
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidatorTime {
    /// Public key of the validator.
    pub public_key: PublicKey,
    /// Time of the validator.
    pub time: Option<DateTime<Utc>>,
}

/// Shortcut to get data from storage.
impl TimeApi {
    /// Endpoint for getting value of the time that is saved in storage.
    fn wire_current_time(self, router: &mut Router) {
        let current_time = move |_: &mut Request| -> IronResult<Response> {
            let view = self.blockchain.snapshot();
            let schema = TimeSchema::new(&view);
            self.ok_response(&json!(schema.time().get()))
        };

        router.get("v1/current_time", current_time, "get_current_time");
    }

    /// Endpoint for getting time values for all validators.
    fn get_all_validators_times(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let schema = TimeSchema::new(&view);
        let idx = schema.validators_times();

        // The times of all validators for which time is known.
        let validators_times = idx.iter()
            .map(|(public_key, time)| ValidatorTime {
                public_key,
                time: Some(time),
            })
            .collect::<Vec<_>>();

        self.ok_response(&serde_json::to_value(validators_times).unwrap())
    }

    /// Endpoint for getting time values for current validators.
    fn get_current_validators_times(&self, _: &mut Request) -> IronResult<Response> {
        let view = self.blockchain.snapshot();
        let validator_keys = Schema::new(&view).actual_configuration().validator_keys;
        let schema = TimeSchema::new(&view);
        let idx = schema.validators_times();

        // The times of current validators.
        // `None` if the time of the validator is unknown.
        let validators_times = validator_keys
            .iter()
            .map(|validator| ValidatorTime {
                public_key: validator.service_key,
                time: idx.get(&validator.service_key),
            })
            .collect::<Vec<_>>();

        self.ok_response(&serde_json::to_value(validators_times).unwrap())
    }

    /// Wiring private node API
    pub fn wire_private(&self, router: &mut Router) {
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
        self.clone().wire_current_time(router);
    }
}

impl fmt::Debug for TimeApi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TimeApi {{}}")
    }
}
