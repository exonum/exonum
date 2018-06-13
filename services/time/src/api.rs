use chrono::{DateTime, Utc};
use exonum::{api::Api,
             blockchain::{Blockchain, Schema},
             crypto::PublicKey,
             encoding::serialize::json::reexport as serde_json};
use iron::prelude::*;
use router::Router;

use super::TimeSchema;

/// Implements the node public API.
#[derive(Clone, Debug)]
pub struct PublicApi {
    /// Exonum blockchain.
    pub blockchain: Blockchain,
}

/// Implements the node private API.
#[derive(Clone, Debug)]
pub struct PrivateApi {
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
impl PublicApi {
    /// Endpoint for getting value of the time that is saved in storage.
    fn wire_current_time(self, router: &mut Router) {
        let current_time = move |_: &mut Request| -> IronResult<Response> {
            let view = self.blockchain.snapshot();
            let schema = TimeSchema::new(&view);
            self.ok_response(&json!(schema.time().get()))
        };

        router.get("v1/current_time", current_time, "get_current_time");
    }
}

impl PrivateApi {
    /// Endpoint for getting time values for all validators.
    fn wire_get_all_validators_times(self, router: &mut Router) {
        let get_all_validators_times = move |_: &mut Request| -> IronResult<Response> {
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
        };

        router.get(
            "v1/validators_times/all",
            get_all_validators_times,
            "get_all_validators_times",
        );
    }

    /// Endpoint for getting time values for current validators.
    fn wire_get_current_validators_times(self, router: &mut Router) {
        let validators_times = move |_: &mut Request| -> IronResult<Response> {
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
        };

        router.get(
            "v1/validators_times",
            validators_times,
            "get_current_validators_times",
        );
    }
}

impl Api for PublicApi {
    fn wire(&self, router: &mut Router) {
        self.clone().wire_current_time(router);
    }
}

impl Api for PrivateApi {
    fn wire(&self, router: &mut Router) {
        self.clone().wire_get_all_validators_times(router);
        self.clone().wire_get_current_validators_times(router);
    }
}

pub mod ng {
    //! Exonum-time API.

    use chrono::{DateTime, Utc};

    use exonum::{api_ng, blockchain::Schema};

    use TimeSchema;

    use super::ValidatorTime;

    /// Implements the exonum-time public API.
    #[derive(Debug)]
    pub struct PublicApi;

    impl PublicApi {
        /// Endpoint for getting time values for all validators.
        pub fn current_time(
            state: &api_ng::ServiceApiState,
            _query: (),
        ) -> api_ng::Result<Option<DateTime<Utc>>> {
            let view = state.blockchain().snapshot();
            let schema = TimeSchema::new(&view);
            Ok(schema.time().get())
        }

        /// Used to extend Api.
        pub fn wire(builder: &mut api_ng::ServiceApiBuilder) {
            builder
                .public_scope()
                .endpoint("v1/current_time", Self::current_time);
        }
    }

    /// Implements the exonum-time private API.
    #[derive(Debug)]
    pub struct PrivateApi;

    impl PrivateApi {
        /// Endpoint for getting time values for all validators.
        pub fn all_validators_times(
            state: &api_ng::ServiceApiState,
            _query: (),
        ) -> api_ng::Result<Vec<ValidatorTime>> {
            let view = state.blockchain().snapshot();
            let schema = TimeSchema::new(&view);
            let idx = schema.validators_times();

            // The times of all validators for which time is known.
            let validators_times = idx.iter()
                .map(|(public_key, time)| ValidatorTime {
                    public_key,
                    time: Some(time),
                })
                .collect::<Vec<_>>();
            Ok(validators_times)
        }

        /// Endpoint for getting time values for current validators.
        pub fn current_validators_time(
            state: &api_ng::ServiceApiState,
            _query: (),
        ) -> api_ng::Result<Vec<ValidatorTime>> {
            let view = state.blockchain().snapshot();
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
            Ok(validators_times)
        }

        /// Used to extend Api.
        pub fn wire(builder: &mut api_ng::ServiceApiBuilder) {
            builder
                .private_scope()
                .endpoint("v1/validators_times", Self::current_validators_time)
                .endpoint("v1/validators_times/all", Self::all_validators_times);
        }
    }
}
