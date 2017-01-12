/// idea behind this file is described in issue-74
/// we want to adjust `propose_timeout` and make it dependent on number of incoming transactions
/// the more transactions are coming - the less should be time between blocks
/// and, in contrast, if almost no transactions are observed - no need to accept blocks often

use std::ops::{Deref, DerefMut};

use ::blockchain::Schema;
use ::storage::View as StorageView;
use ::storage::{List, Map};

pub type Timeout = i64;//todo wait until sandbox-tests are merged and use type from config.rs
pub type BlockSize = usize;
pub type Float = f64;


pub trait ProposeTimeoutAdjuster {
    fn update_last_propose_timeout(&mut self, new_last_propose_timeout: Timeout);

    fn adjusted_propose_timeout(&self, view: &StorageView) -> Timeout;
}

impl<F: ?Sized> ProposeTimeoutAdjuster for Box<F>
    where F: ProposeTimeoutAdjuster
{
    fn update_last_propose_timeout(&mut self, new_last_propose_timeout: Timeout) {
        self.deref_mut().update_last_propose_timeout(new_last_propose_timeout)
    }
    fn adjusted_propose_timeout(&self, view: &StorageView) -> Timeout {
        self.deref().adjusted_propose_timeout(view)
    }
}

pub struct ConstProposeTimeout {
    pub propose_timeout: Timeout,
}

impl Default for ConstProposeTimeout {
    fn default() -> Self {
        ConstProposeTimeout { propose_timeout: 200 }
    }
}

impl ProposeTimeoutAdjuster for ConstProposeTimeout {
    fn update_last_propose_timeout(&mut self, _new_last_propose_timeout: Timeout) {}

    fn adjusted_propose_timeout(&self, _view: &StorageView) -> Timeout {
        self.propose_timeout
    }
}

pub struct MovingAverageProposeTimeoutAdjuster {
    pub propose_timeout_min: Timeout,
    pub propose_timeout_max: Timeout,
    pub speed_of_adjustment: Float,
    pub target_block_portion_feel: Float,
    pub desired_block_size_max: BlockSize,

    pub last_propose_timeout: Timeout,
}

impl Default for MovingAverageProposeTimeoutAdjuster {
    fn default() -> Self {
        MovingAverageProposeTimeoutAdjuster {
            // todo move default values to config
            propose_timeout_min: 50,
            propose_timeout_max: 200,
            speed_of_adjustment: 0.7,
            target_block_portion_feel: 0.7,
            desired_block_size_max: 3000,
            last_propose_timeout: 200,
        }
    }
}

impl ProposeTimeoutAdjuster for MovingAverageProposeTimeoutAdjuster {
    fn adjusted_propose_timeout(&self, view: &StorageView) -> Timeout {
        let schema = Schema::new(view);

        let last_block_hash = schema.heights().last().unwrap_or(None);
        let last_height = match last_block_hash {
            Some(hash) => schema.blocks().get(&hash).unwrap_or(None).map_or(0, |b| b.height()),
            None => 0,
        };
        let last_block_size = schema.block_txs(last_height).len().unwrap_or(0);

        {
            // calculate adjusted_propose_time using last_block_size and stored last_propose_timeout

            // target_delta_t = DELTA_T_MAX - (DELTA_T_MAX - DELTA_T_MIN) * min(1, block / (ALPHA * MAX_BLOCK))

            // calculate above formula by parts:

            // min(1, block / (ALPHA * MAX_BLOCK))
            let block_filling_rate: Float = (1 as Float)
                .min(last_block_size as Float /
                     (self.target_block_portion_feel * self.desired_block_size_max as Float));

            //(DELTA_T_MAX - DELTA_T_MIN)
            let adjusted_propose_timeout_range: Timeout = self.propose_timeout_max -
                                                          self.propose_timeout_min;

            // collect parts in original formula:
            let target_propose_timeout: Float = (self.propose_timeout_max as Float) +
                                                adjusted_propose_timeout_range as Float *
                                                block_filling_rate;

            // delta_t = delta_t * BETA + (1-BETA) * target_delta_t
            let adjusted_propose_timeout: Float =
                target_propose_timeout as Float * self.speed_of_adjustment +
                self.last_propose_timeout as Float * ((1 as Float) - self.speed_of_adjustment);

            adjusted_propose_timeout as Timeout
        }
    }

    /// setter for last_propose_timeout
    /// by design, should be used only when new propose_timeout is created
    /// whereas getter can be called without any objections
    fn update_last_propose_timeout(&mut self, new_last_propose_timeout: Timeout) {
        self.last_propose_timeout = new_last_propose_timeout;
    }
}
