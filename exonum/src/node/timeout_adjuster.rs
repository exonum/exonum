use events::Milliseconds;
use node::State;
use storage::View;

/// `TimeoutAdjuster` trait can be used to dynamically change propose timeout.
///
/// # Examples
///
/// Implementing `TimeoutAdjuster`:
///
/// ```
/// use exonum::node::State;
/// use exonum::node::timeout_adjuster::TimeoutAdjuster;
/// use exonum::events::Milliseconds;
/// use exonum::storage::View;
///
/// # #[allow(dead_code)]
/// struct CustomAdjuster {}
///
/// impl TimeoutAdjuster for CustomAdjuster {
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
pub trait TimeoutAdjuster: Send {
    /// Called during node initialization and after accepting a new height.
    fn adjust_timeout(&mut self, state: &State, view: View) -> Milliseconds;
}

/// `Adjuster` implementation that returns value of `propose_timeout` field from `ConsensusConfig`.
#[derive(Default, Debug)]
pub struct Constant {}

impl TimeoutAdjuster for Constant {
    fn adjust_timeout(&mut self, state: &State, _: View) -> Milliseconds {
        state.consensus_config().propose_timeout
    }
}

/// Moving average timeout calculation. Initial timeout is equal to `min_timeout`.
#[derive(Debug)]
pub struct MovingAverage {
    adjustment_speed: f64,
    optimal_block_load: f64,
    min_timeout: f64,
    max_timeout: f64,
    previous_timeout: f64,
}

impl MovingAverage {
    /// Creates `MovingAverage` that generates values between `min_timeout` and `max_timeout`.
    pub fn new(adjustment_speed: f64,
               optimal_block_load: f64,
               min_timeout: Milliseconds,
               max_timeout: Milliseconds) -> Self {
        MovingAverage {
            adjustment_speed: adjustment_speed,
            optimal_block_load: optimal_block_load,
            min_timeout: min_timeout as f64,
            max_timeout: max_timeout as f64,
            previous_timeout: min_timeout as f64,
        }
    }

    fn adjust_timeout_impl(&mut self,
                           txs_block_limit: f64,
                           current_load: f64) -> Milliseconds {
        let optimal_load = txs_block_limit * self.optimal_block_load;
        let load_percent = current_load / optimal_load;

        let target_t = if current_load < optimal_load {
            self.max_timeout - (self.max_timeout - self.previous_timeout) * load_percent
        } else {
            self.previous_timeout - (self.previous_timeout - self.min_timeout) *
                (load_percent - 1.) / (1. / self.optimal_block_load - 1.)
        };

        self.previous_timeout = target_t * self.adjustment_speed + self.previous_timeout *
            (1. - self.adjustment_speed);
        self.previous_timeout.round() as Milliseconds
    }
}

impl TimeoutAdjuster for MovingAverage {
    fn adjust_timeout(&mut self, state: &State, _: View) -> Milliseconds {
        self.adjust_timeout_impl(state.config().consensus.txs_block_limit as f64,
                                 state.transactions().len() as f64)
    }
}

#[cfg(test)]
mod tests {
    use env_logger;

    use super::*;
    use events::Milliseconds;

    #[test]
    fn moving_average_timeout_adjuster() {
        let _ = env_logger::init();

        static MIN_TIMEOUT: Milliseconds = 1;
        static MAX_TIMEOUT: Milliseconds = 10000;
        static TXS_BLOCK_LIMIT: f64 = 5000.;
        static TEST_COUNT: usize = 10;

        let mut adjuster = MovingAverage::new(0.75, 0.7, MIN_TIMEOUT, MAX_TIMEOUT);

        // Timeout should stay minimal if there are `TXS_BLOCK_LIMIT` or more transactions.
        for _ in 0..TEST_COUNT {
            assert_eq!(MIN_TIMEOUT, adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT, TXS_BLOCK_LIMIT));
        }

        for _ in 0..TEST_COUNT {
            assert_eq!(MIN_TIMEOUT, adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT,
                                                                 TXS_BLOCK_LIMIT * 2.));
        }

        static TXS_TEST_DATA: &'static [f64] = &[0., 100., 200., 300., 400., 500., 1000., 1500.,
            2000., 2500., 3000., 4000., 4500., 5000., 5500., 6000.];

        // As the transaction number declines, timeout should increase until it reaches maximum.
        let mut previous_timeout = adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT, TXS_BLOCK_LIMIT);
        for transactions in TXS_TEST_DATA.iter().rev() {
            let timeout = adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT, *transactions);
            info!("Timeout: current = {}, previous = {}", timeout, previous_timeout);
            assert!(timeout >= previous_timeout);
            previous_timeout = timeout;
        }

        // Timeout should stay maximal if there are no transactions for some time.
        for _ in 0..TEST_COUNT {
            assert_eq!(MAX_TIMEOUT, adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT, 0.));
        }

        // As the transactions number increases, timeout should decrease until it reaches minimum.
        let mut previous_timeout = adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT, 0.);
        for transactions in TXS_TEST_DATA {
            let timeout = adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT, *transactions);
            info!("Timeout: current = {}, previous = {}", timeout, previous_timeout);
            assert!(timeout <= previous_timeout);
            previous_timeout = timeout;
        }
        assert_eq!(MIN_TIMEOUT, adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT, TXS_BLOCK_LIMIT));
    }
}
