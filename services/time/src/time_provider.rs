use chrono::{DateTime, Duration, TimeZone, Utc};
use std::sync::{Arc, RwLock};

/// A helper trait that provides the node with a current time.
pub trait TimeProvider: Send + Sync + ::std::fmt::Debug {
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
/// # extern crate exonum;
/// # extern crate exonum_testkit;
/// # extern crate exonum_time;
/// # extern crate chrono;
/// use chrono::{Utc, Duration, TimeZone};
/// use exonum::helpers::Height;
/// use exonum_testkit::TestKitBuilder;
/// use exonum_time::{time_provider::MockTimeProvider, schema::TimeSchema, TimeService};
///
/// # fn main() {
/// let mock_provider = MockTimeProvider::default();
/// let mut testkit = TestKitBuilder::validator()
///     .with_service(TimeService::with_provider(mock_provider.clone()))
///     .create();
/// mock_provider.add_time(Duration::seconds(15));
/// testkit.create_blocks_until(Height(2));
///
/// // The time reported by the mock time provider is reflected by the service.
/// let snapshot = testkit.snapshot();
/// let schema = TimeSchema::new(snapshot);
/// assert_eq!(
///     Some(Utc.timestamp(15, 0)),
///     schema.time().get().map(|time| time)
/// );
/// # }
/// ```
///
/// [`Arc`]: https://doc.rust-lang.org/std/sync/struct.Arc.html
/// [`TimeService`]: ../struct.TimeService.html
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

impl From<MockTimeProvider> for Box<TimeProvider> {
    fn from(mock_time_provider: MockTimeProvider) -> Self {
        Box::new(mock_time_provider) as Box<TimeProvider>
    }
}
