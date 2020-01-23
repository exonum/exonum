// Copyright 2019 The Exonum Team
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

//! Sample counter service.
use exonum::{
    merkledb::{
        access::{Access, FromAccess, RawAccessMut},
        ProofEntry,
    },
    runtime::{ExecutionContext, ExecutionError, InstanceId},
};
use exonum_derive::*;
use exonum_rust_runtime::{DefaultInstance, Service};

pub const SERVICE_NAME: &str = "counter";
pub const SERVICE_ID: InstanceId = 100;

#[derive(FromAccess)]
pub struct CounterSchema<T: Access> {
    pub counter: ProofEntry<T::Base, u64>,
}

impl<T: Access> CounterSchema<T> {
    fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

impl<T> CounterSchema<T>
where
    T: Access,
    T::Base: RawAccessMut,
{
    fn inc_counter(&mut self, inc: u64) -> u64 {
        let count = self
            .counter
            .get()
            .unwrap_or(0)
            .checked_add(inc)
            .expect("attempt to add with overflow");
        self.counter.set(count);
        count
    }
}

// // // // Transactions // // // //

#[derive(Debug, ExecutionFail)]
pub enum Error {
    /// Adding zero does nothing!
    AddingZero = 0,
    /// What's the question?
    AnswerToTheUltimateQuestion = 1,
    /// Number 13 is considered unlucky by some cultures.
    BadLuck = 2,
}

#[exonum_interface(auto_ids)]
pub trait CounterInterface<Ctx> {
    type Output;

    // This method purposely does not check counter overflow in order to test
    // behavior of panicking transactions.
    fn increment(&self, ctx: Ctx, by: u64) -> Self::Output;
    fn reset(&self, ctx: Ctx, _: ()) -> Self::Output;
}

impl CounterInterface<ExecutionContext<'_>> for CounterService {
    type Output = Result<(), ExecutionError>;

    fn increment(&self, context: ExecutionContext<'_>, by: u64) -> Self::Output {
        if by == 0 {
            return Err(Error::AddingZero.into());
        }

        let mut schema = CounterSchema::new(context.service_data());
        schema.inc_counter(by);
        Ok(())
    }

    fn reset(&self, context: ExecutionContext<'_>, _: ()) -> Self::Output {
        let mut schema = CounterSchema::new(context.service_data());
        schema.counter.set(0);
        Ok(())
    }
}

// // // // Service // // // //

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "counter-service", artifact_version = "1.0.0")]
#[service_dispatcher(implements("CounterInterface"))]
pub struct CounterService;

impl DefaultInstance for CounterService {
    const INSTANCE_ID: u32 = SERVICE_ID;
    const INSTANCE_NAME: &'static str = SERVICE_NAME;
}

impl Service for CounterService {
    fn before_transactions(&self, context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        let mut schema = CounterSchema::new(context.service_data());
        if schema.counter.get() == Some(13) {
            schema.counter.set(0);
            Err(Error::BadLuck.into())
        } else {
            Ok(())
        }
    }

    fn after_transactions(&self, context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        let schema = CounterSchema::new(context.service_data());
        if schema.counter.get() == Some(42) {
            Err(Error::AnswerToTheUltimateQuestion.into())
        } else {
            Ok(())
        }
    }
}
