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

//! `TimeoutAdjuster` is used to dynamically change propose timeout.

use std::fmt::Debug;

use helpers::Milliseconds;
use storage::Snapshot;
use blockchain::Schema;

/// `TimeoutAdjuster` trait is used to dynamically change propose timeout.
///
/// # Examples
///
/// Implementing `TimeoutAdjuster`:
///
/// ```
/// use exonum::node::timeout_adjuster::TimeoutAdjuster;
/// use exonum::helpers::Milliseconds;
/// use exonum::storage::Snapshot;
/// use exonum::blockchain::Schema;
///
/// # #[allow(dead_code)]
/// # #[derive(Debug)]
/// struct CustomAdjuster {}
///
/// impl TimeoutAdjuster for CustomAdjuster {
///     fn adjust_timeout(&mut self, snapshot: &Snapshot) -> Milliseconds {
///         let schema = Schema::new(snapshot);
///         let transactions = schema.last_block().map_or(0, |block| block.tx_count());
///         // Simply increase propose time after empty blocks.
///         if transactions == 0 {
///             1000
///         } else {
///             200
///         }
///     }
/// }
/// ```
/// For more examples see `Constant`, `Dynamic` and `MovingAverage` implementations.
pub trait TimeoutAdjuster: Send + Debug {
    /// Called during node initialization and after accepting a new height.
    fn adjust_timeout(&mut self, view: &Snapshot) -> Milliseconds;
}

/// `Adjuster` implementation that always returns the same value.
#[derive(Debug)]
pub struct Constant {
    timeout: Milliseconds,
}

impl Constant {
    /// Creates a new `Constant` timeout adjuster instance with the given timeout.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use exonum::node::timeout_adjuster::Constant;
    ///
    /// let mut adjuster = Constant::new(10);
    /// # drop(adjuster);
    /// ```
    pub fn new(timeout: Milliseconds) -> Self {
        Constant { timeout }
    }
}

impl TimeoutAdjuster for Constant {
    fn adjust_timeout(&mut self, _: &Snapshot) -> Milliseconds {
        self.timeout
    }
}

/// `Adjuster` implementation that returns minimal or maximal timeout value depending on the
/// transactions amount in the previous block.
#[derive(Debug, Default)]
pub struct Dynamic {
    min: Milliseconds,
    max: Milliseconds,
    threshold: u32,
}

impl Dynamic {
    /// Creates a new `Dynamic` timeout adjuster instance with given parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use exonum::node::timeout_adjuster::Dynamic;
    ///
    /// let mut adjuster = Dynamic::new(1, 10, 100);
    /// # drop(adjuster);
    /// ```
    pub fn new(min: Milliseconds, max: Milliseconds, threshold: u32) -> Self {
        Dynamic {
            min,
            max,
            threshold,
        }
    }

    fn adjust_timeout_impl(&mut self, current_load: u32) -> Milliseconds {
        if current_load >= self.threshold {
            self.min
        } else {
            self.max
        }
    }
}

impl TimeoutAdjuster for Dynamic {
    fn adjust_timeout(&mut self, snapshot: &Snapshot) -> Milliseconds {
        let schema = Schema::new(snapshot);
        let threshold = self.threshold;
        self.adjust_timeout_impl(schema.last_block().map_or(
            threshold,
            |block| block.tx_count(),
        ))
    }
}

/// Moving average timeout calculation. Initial timeout is equal to `min_timeout`.
#[derive(Debug)]
pub struct MovingAverage {
    min: f64,
    max: f64,
    adjustment_speed: f64,
    txs_block_limit: f64,
    optimal_block_load: f64,
    previous_timeout: f64,
}

impl MovingAverage {
    /// Creates `MovingAverage` that generates values between `min_timeout` and `max_timeout`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #![allow(unused_mut)]
    /// use exonum::node::timeout_adjuster::MovingAverage;
    ///
    /// let mut adjuster = MovingAverage::new(1, 10, 0.5, 5000, 0.75);
    /// # drop(adjuster);
    /// ```
    pub fn new(
        min: Milliseconds,
        max: Milliseconds,
        adjustment_speed: f64,
        txs_block_limit: u32,
        optimal_block_load: f64,
    ) -> Self {
        MovingAverage {
            min: min as f64,
            max: max as f64,
            adjustment_speed,
            txs_block_limit: f64::from(txs_block_limit),
            optimal_block_load,
            previous_timeout: min as f64,
        }
    }

    fn adjust_timeout_impl(&mut self, current_load: f64) -> Milliseconds {
        let optimal_load = self.txs_block_limit * self.optimal_block_load;
        let load_percent = current_load / optimal_load;

        let target_t = if current_load < optimal_load {
            self.max - (self.max - self.previous_timeout) * load_percent
        } else {
            self.previous_timeout -
                (self.previous_timeout - self.min) * (load_percent - 1.) /
                    (1. / self.optimal_block_load - 1.)
        };

        self.previous_timeout = target_t * self.adjustment_speed +
            self.previous_timeout * (1. - self.adjustment_speed);
        self.previous_timeout.round() as Milliseconds
    }
}

impl TimeoutAdjuster for MovingAverage {
    fn adjust_timeout(&mut self, snapshot: &Snapshot) -> Milliseconds {
        let schema = Schema::new(snapshot);
        self.adjust_timeout_impl(schema.last_block().map_or(
            0.,
            |block| f64::from(block.tx_count()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helpers::Milliseconds;

    #[test]
    fn dynamic_timeout_adjuster() {
        static MIN_TIMEOUT: Milliseconds = 1;
        static MAX_TIMEOUT: Milliseconds = 10;
        static THRESHOLD: u32 = 2;

        let test_data = [
            (0, MAX_TIMEOUT),
            (1, MAX_TIMEOUT),
            (2, MIN_TIMEOUT),
            (3, MIN_TIMEOUT),
            (10, MIN_TIMEOUT),
            (100, MIN_TIMEOUT),
        ];

        let mut adjuster = Dynamic::new(MIN_TIMEOUT, MAX_TIMEOUT, THRESHOLD);

        for data in &test_data {
            assert_eq!(data.1, adjuster.adjust_timeout_impl(data.0));
        }
    }

    #[test]
    fn moving_average_timeout_adjuster() {

        static MIN_TIMEOUT: Milliseconds = 1;
        static MAX_TIMEOUT: Milliseconds = 10000;
        static TXS_BLOCK_LIMIT: f64 = 5000.;
        static TEST_COUNT: usize = 10;

        let mut adjuster =
            MovingAverage::new(MIN_TIMEOUT, MAX_TIMEOUT, 0.75, TXS_BLOCK_LIMIT as u32, 0.7);

        // Timeout should stay minimal if there are `TXS_BLOCK_LIMIT` or more transactions.
        for _ in 0..TEST_COUNT {
            assert_eq!(MIN_TIMEOUT, adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT));
        }

        for _ in 0..TEST_COUNT {
            assert_eq!(
                MIN_TIMEOUT,
                adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT * 2.)
            );
        }

        static TXS_TEST_DATA: &'static [f64] = &[
            0.,
            100.,
            200.,
            300.,
            400.,
            500.,
            1000.,
            1500.,
            2000.,
            2500.,
            3000.,
            4000.,
            4500.,
            5000.,
            5500.,
            6000.,
        ];

        // As the transaction number declines, timeout should increase until it reaches maximum.
        let mut previous_timeout = adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT);
        for transactions in TXS_TEST_DATA.iter().rev() {
            let timeout = adjuster.adjust_timeout_impl(*transactions);
            info!(
                "Timeout: current = {}, previous = {}",
                timeout,
                previous_timeout
            );
            assert!(timeout >= previous_timeout);
            previous_timeout = timeout;
        }

        // Timeout should stay maximal if there are no transactions for some time.
        for _ in 0..TEST_COUNT {
            assert_eq!(MAX_TIMEOUT, adjuster.adjust_timeout_impl(0.));
        }

        // As the transactions number increases, timeout should decrease until it reaches minimum.
        let mut previous_timeout = adjuster.adjust_timeout_impl(0.);
        for transactions in TXS_TEST_DATA {
            let timeout = adjuster.adjust_timeout_impl(*transactions);
            info!(
                "Timeout: current = {}, previous = {}",
                timeout,
                previous_timeout
            );
            assert!(timeout <= previous_timeout);
            previous_timeout = timeout;
        }
        assert_eq!(MIN_TIMEOUT, adjuster.adjust_timeout_impl(TXS_BLOCK_LIMIT));
    }
}
