/// idea behind this service is described in issue-74
/// we want to adjust `propose_timeout` and make it dependent on number of incoming transactions
/// the more transactions are coming - the less should be time between blocks
/// and, in contrast, if almost no transactions are observed - no need to accept blocks often
// FIXME! Use last propose time instead last propose timeout
#[macro_use]
extern crate exonum;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

use serde_json::value::{ToJson, Value, from_value};

use exonum::blockchain::{Schema, Service, NodeState, Transaction};
use exonum::storage::{View, List, Map, Error as StorageError};
use exonum::messages::{RawTransaction, Error as MessageError};

pub const PROPOSE_TIMEOUT_ADJUST_SERVICE: u16 = 2;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProposeTimeoutAdjusterConfig {
    pub propose_timeout_min: i64,
    pub propose_timeout_max: i64,
    pub speed_of_adjustment: f64,
    pub target_block_portion_feel: f64,
    pub desired_block_size_max: u64,
}

impl Default for ProposeTimeoutAdjusterConfig {
    fn default() -> Self {
        ProposeTimeoutAdjusterConfig {
            propose_timeout_min: 50,
            propose_timeout_max: 200,
            speed_of_adjustment: 0.7,
            target_block_portion_feel: 0.7,
            desired_block_size_max: 3000,
        }
    }
}

pub struct ProposeTimeoutAdjusterService {
    cfg: ProposeTimeoutAdjusterConfig,
}

impl ProposeTimeoutAdjusterService {
    pub fn new(cfg: ProposeTimeoutAdjusterConfig) -> ProposeTimeoutAdjusterService {
        ProposeTimeoutAdjusterService { cfg: cfg }
    }

    pub fn adjusted_propose_timeout(&self, state: &NodeState) -> Result<i64, StorageError> {
        let schema = Schema::new(state.view());
        let cfg: ProposeTimeoutAdjusterConfig = from_value(state.service_config(self).clone())
            .unwrap();

        let last_block_hash = schema.heights().last()?;
        let last_height = match last_block_hash {
            Some(hash) => schema.blocks().get(&hash)?.map_or(0, |b| b.height()),
            None => 0,
        };
        let last_block_size = schema.block_txs(last_height).len()?;

        {
            // calculate adjusted_propose_time using last_block_size and stored last_propose_timeout

            // target_delta_t = DELTA_T_MAX - (DELTA_T_MAX - DELTA_T_MIN) * min(1, block / (ALPHA * MAX_BLOCK))

            // calculate above formula by parts:

            // min(1, block / (ALPHA * MAX_BLOCK))
            let block_filling_rate: f64 = (1 as f64).min(last_block_size as f64 /
                                                         (cfg.target_block_portion_feel *
                                                          cfg.desired_block_size_max as f64));

            //(DELTA_T_MAX - DELTA_T_MIN)
            let adjusted_propose_timeout_range: i64 = cfg.propose_timeout_max -
                                                      cfg.propose_timeout_min;

            // collect parts in original formula:
            let target_propose_timeout: f64 = (cfg.propose_timeout_max as f64) +
                                              adjusted_propose_timeout_range as f64 *
                                              block_filling_rate;

            // delta_t = delta_t * BETA + (1-BETA) * target_delta_t
            let adjusted_propose_timeout: f64 =
                target_propose_timeout as f64 * cfg.speed_of_adjustment +
                state.propose_timeout() as f64 * ((1 as f64) - cfg.speed_of_adjustment);

            Ok(adjusted_propose_timeout as i64)
        }
    }
}

impl Service for ProposeTimeoutAdjusterService {
    fn service_id(&self) -> u16 {
        PROPOSE_TIMEOUT_ADJUST_SERVICE
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        Err(MessageError::IncorrectMessageType { message_type: raw.message_type() })
    }

    fn handle_genesis_block(&self, _: &View) -> Result<Value, StorageError> {
        Ok(self.cfg.to_json())
    }

    fn handle_commit(&self, state: &mut NodeState) -> Result<(), StorageError> {
        let new_timeout = self.adjusted_propose_timeout(state)?;
        trace!("New propose timeout={}ms", new_timeout);
        state.set_propose_timeout(new_timeout);
        Ok(())
    }
}
