// Copyright 2020 The Exonum Team
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

use chrono::{DateTime, Duration, TimeZone, Utc};

use std::sync::{Arc, RwLock};

/// A helper trait that provides the node with a current time.
pub trait TimeProvider: Send + Sync + std::fmt::Debug {
    /// Returns the current time.
    fn current_time(&self) -> DateTime<Utc>;
}

#[derive(Debug)]
/// Provider of system time.
pub struct SystemTimeProvider;

impl TimeProvider for SystemTimeProvider {
    fn current_time(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Mock time provider for service testing.
///
/// In terms of use, the mock time provider is similar to [`Arc`]; that is, clones of the provider
/// control the same time record as the original instance. Therefore, to use the mock provider,
/// one may clone its instance and use the clone to construct a [`TimeService`],
/// while keeping the original instance to adjust the time reported to the validators
/// along various test scenarios.
///
/// # Examples
///
/// ```
/// use chrono::{Utc, Duration, TimeZone};
/// use exonum::{helpers::Height, runtime::SnapshotExt};
/// use exonum_testkit::TestKit;
/// use exonum_time::{MockTimeProvider, TimeSchema, TimeServiceFactory};
///
/// # fn main() {
/// let service_name = "time";
/// let service_id = 12;
///
/// let mock_provider = MockTimeProvider::default();
/// let mut testkit = TestKit::for_rust_service(
///     TimeServiceFactory::with_provider(mock_provider.clone()),
///     service_name,
///     service_id,
///     ()
/// );
/// mock_provider.add_time(Duration::seconds(15));
/// testkit.create_blocks_until(Height(2));
///
/// // The time reported by the mock time provider is reflected by the service.
/// let snapshot = testkit.snapshot();
/// let schema: TimeSchema<_> = snapshot.service_schema(service_name).unwrap();
/// assert_eq!(
///     Utc.timestamp(15, 0),
///     schema.time.get().unwrap()
/// );
/// # }
/// ```
///
/// [`Arc`]: https://doc.rust-lang.org/std/sync/struct.Arc.html
/// [`TimeService`]: struct.TimeService.html
#[derive(Debug, Clone)]
pub struct MockTimeProvider {
    /// Local time value.
    time: Arc<RwLock<DateTime<Utc>>>,
}

impl Default for MockTimeProvider {
    /// Initializes the provider with the time set to the Unix epoch start.
    fn default() -> Self {
        Self::new(Utc.timestamp(0, 0))
    }
}

impl MockTimeProvider {
    /// Creates a new `MockTimeProvider` with time value equal to `time`.
    pub fn new(time: DateTime<Utc>) -> Self {
        Self {
            time: Arc::new(RwLock::new(time)),
        }
    }

    /// Gets the time value currently reported by the provider.
    pub fn time(&self) -> DateTime<Utc> {
        *self.time.read().unwrap()
    }

    /// Sets the time value to `new_time`.
    pub fn set_time(&self, new_time: DateTime<Utc>) {
        let mut time = self.time.write().unwrap();
        *time = new_time;
    }

    /// Adds `duration` to the value of `time`.
    pub fn add_time(&self, duration: Duration) {
        let mut time = self.time.write().unwrap();
        *time = *time + duration;
    }
}

impl TimeProvider for MockTimeProvider {
    fn current_time(&self) -> DateTime<Utc> {
        self.time()
    }
}

impl From<MockTimeProvider> for Arc<dyn TimeProvider> {
    fn from(time_provider: MockTimeProvider) -> Self {
        Arc::new(time_provider) as Arc<dyn TimeProvider>
    }
}

impl From<SystemTimeProvider> for Arc<dyn TimeProvider> {
    fn from(time_provider: SystemTimeProvider) -> Self {
        Arc::new(time_provider) as Arc<dyn TimeProvider>
    }
}
