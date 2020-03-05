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

use futures::{TryFuture, TryFutureExt};
use tokio::time::delay_for;

use std::time::Duration;

/// Retries the specified fallible future with the delay strategy specified by the `timeouts`
/// iterator.
pub async fn retry_future<Fut>(
    mut timeouts: impl Iterator<Item = Duration>,
    mut future_fn: impl FnMut() -> Fut,
) -> Result<Fut::Ok, Fut::Error>
where
    Fut: TryFuture,
{
    let timeouts = timeouts.by_ref();
    loop {
        match future_fn().into_future().await {
            Ok(output) => return Ok(output),
            Err(err) => {
                let timeout = timeouts.next().ok_or(err)?;
                delay_for(timeout).await;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct FixedInterval {
    interval: Duration,
}

impl FixedInterval {
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

impl Iterator for FixedInterval {
    type Item = Duration;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.interval)
    }
}
