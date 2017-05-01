use events::Milliseconds;
use node::State;

const MIN_TIMEOUT: Milliseconds = 50;
const MAX_TIMEOUT: Milliseconds = 200;

/// `TimeoutAdjuster` trait can be used to dynamically change propose timeout.
///
/// # Examples
///
/// Implementing `TimeoutAdjuster`:
///
/// ```
/// use exonum::node::{State, TimeoutAdjuster};
/// use exonum::events::Milliseconds;
///
/// struct Adjuster {}
///
/// impl TimeoutAdjuster for Adjuster {
///     fn adjust_timeout(&mut self, state: &State) -> Milliseconds {
///         // Simply increase propose time after empty blocks.
///         if state.transactions().is_empty() {
///             1000
///         } else {
///             200
///         }
///     }
/// }
/// ```
/// For more examples see `ConstantTimeout` and `DynamicTimeout` implementations.
pub trait TimeoutAdjuster {
    /// Called after accepting a new height.
    fn adjust_timeout(&mut self, state: &State) -> Milliseconds;
}

/// `TimeoutAdjuster` implementation that always returns the same value.
pub struct ConstantTimeout {
    timeout: Milliseconds,
}

impl ConstantTimeout {
    /// Creates `ConstantTimeout` with given timeout value.
    pub fn new(timeout: Milliseconds) -> Self {
        ConstantTimeout { timeout: timeout }
    }
}

impl Default for ConstantTimeout {
    fn default() -> Self {
        ConstantTimeout{ timeout: MAX_TIMEOUT }
    }
}

impl TimeoutAdjuster for ConstantTimeout {
    fn adjust_timeout(&mut self, _: &State) -> Milliseconds {
        self.timeout
    }
}

/// Calculates propose timeout using last block size and precious timeout.
///
/// `target_delta_t =
///     DELTA_T_MAX - (DELTA_T_MAX - DELTA_T_MIN) * min(1, block  / (ALPHA * MAX_BLOCK))`
pub struct DynamicTimeout {
    min_timeout: Milliseconds,
    max_timeout: Milliseconds,
    adjustment_speed: f64,
    target_block_portion_feel: f64,
    desired_max_block_size: u64,
}

impl DynamicTimeout {
    /// Created `DynamicTimeout` with default values.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for DynamicTimeout {
    fn default() -> Self {
        DynamicTimeout{
            min_timeout: MIN_TIMEOUT,
            max_timeout: MAX_TIMEOUT,
            adjustment_speed: 0.7,
            target_block_portion_feel: 0.7,
            desired_max_block_size: 3000,
        }
    }
}

impl TimeoutAdjuster for DynamicTimeout {
    fn adjust_timeout(&mut self, state: &State) -> Milliseconds {
        let last_block_size = state.transactions().len() as f64;

        // min(1, block  / (ALPHA * MAX_BLOCK))
        let filling_rate = 1f64.min(last_block_size /
            (self.target_block_portion_feel * self.desired_max_block_size as f64));

        // (DELTA_T_MAX - DELTA_T_MIN)
        let adjusted_timeout = (self.max_timeout - self.min_timeout) as f64;

        // Collects parts of the original formula.
        let target_timeout = self.max_timeout as f64 + adjusted_timeout * filling_rate;

        // delta_t = delta_t * BETA + (1 - BETA) * target_delta_t
        (target_timeout * self.adjustment_speed + state.propose_timeout() as f64
            * (1f64 - self.adjustment_speed)) as Milliseconds
    }
}
