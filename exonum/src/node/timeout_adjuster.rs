use events::Milliseconds;
use node::State;
use storage::View;

const MIN_TIMEOUT: Milliseconds = 50;
const MAX_TIMEOUT: Milliseconds = 200;

/// `Adjuster` trait can be used to dynamically change propose timeout.
///
/// # Examples
///
/// Implementing `Adjuster`:
///
/// ```
/// use exonum::node::State;
/// use exonum::node::timeout_adjuster::Adjuster;
/// use exonum::events::Milliseconds;
/// use exonum::storage::View;
///
/// struct CustomAdjuster {}
///
/// impl Adjuster for CustomAdjuster {
///     fn adjust_timeout(&mut self, state: &State, _: View) -> Milliseconds {
///         // Simply increase propose time after empty blocks.
///         if state.transactions().is_empty() {
///             1000
///         } else {
///             200
///         }
///     }
/// }
/// ```
/// For more examples see `Constant` and `MovingAverage` implementations.
pub trait Adjuster {
    /// Called during node initialization and after accepting a new height.
    fn adjust_timeout(&mut self, state: &State, view: View) -> Milliseconds;
}

/// `Adjuster` implementation that always returns the same value.
pub struct Constant {
    timeout: Milliseconds,
}

impl Constant {
    /// Creates `Constant` with given timeout value.
    pub fn new(timeout: Milliseconds) -> Self {
        Constant { timeout: timeout }
    }
}

impl Default for Constant {
    fn default() -> Self {
        Constant{ timeout: MAX_TIMEOUT }
    }
}

impl Adjuster for Constant {
    fn adjust_timeout(&mut self, _: &State, _: View) -> Milliseconds {
        self.timeout
    }
}

/// Calculates propose timeout using last block size and precious timeout.
///
/// `target_delta_t =
///     DELTA_T_MAX - (DELTA_T_MAX - DELTA_T_MIN) * min(1, block  / (ALPHA * MAX_BLOCK))`
pub struct MovingAverage {
    min_timeout: Milliseconds,
    max_timeout: Milliseconds,
    adjustment_speed: f64,
    target_block_portion_feel: f64,
    desired_max_block_size: u64,
}

impl MovingAverage {
    /// Created `DynamicTimeout` with default values.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for MovingAverage {
    fn default() -> Self {
        MovingAverage{
            min_timeout: MIN_TIMEOUT,
            max_timeout: MAX_TIMEOUT,
            adjustment_speed: 0.7,
            target_block_portion_feel: 0.7,
            desired_max_block_size: 3000,
        }
    }
}

impl Adjuster for MovingAverage {
    fn adjust_timeout(&mut self, state: &State, _: View) -> Milliseconds {
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
