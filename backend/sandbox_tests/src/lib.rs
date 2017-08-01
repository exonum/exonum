#[macro_use]
extern crate log;
extern crate serde;
extern crate serde_json;
extern crate iron;
extern crate iron_test;
extern crate mime;
extern crate router;
#[macro_use]
extern crate exonum;
extern crate sandbox;
extern crate timestamping;

mod api_tests;

use std::ops::{Deref, DerefMut};
use std::cell::{Ref, RefCell};

use exonum::messages::Message;
use sandbox::sandbox::{Sandbox, sandbox_with_services};
use sandbox::sandbox_tests_helper::add_one_height_with_transactions;
use sandbox::sandbox_tests_helper::{SandboxState, VALIDATOR_0};
use timestamping::TimestampingService;

pub struct TimestampingSandbox {
    inner: Sandbox,
    state: RefCell<SandboxState>,
}

impl Deref for TimestampingSandbox {
    type Target = Sandbox;

    fn deref(&self) -> &Sandbox {
        &self.inner
    }
}

impl DerefMut for TimestampingSandbox {
    fn deref_mut(&mut self) -> &mut Sandbox {
        &mut self.inner
    }
}

impl Default for TimestampingSandbox {
    fn default() -> TimestampingSandbox {
        TimestampingSandbox::new()
    }
}

impl TimestampingSandbox {
    pub fn new() -> TimestampingSandbox {
        let sandbox = sandbox_with_services(vec![Box::new(TimestampingService::new())]);

        info!("Sandbox tests inited");

        TimestampingSandbox {
            inner: sandbox,
            state: SandboxState::new().into(),
        }
    }

    pub fn state_ref(&self) -> Ref<SandboxState> {
        self.state.borrow()
    }

    pub fn add_height_with_tx<T: Message>(&self, tx: T) {
        add_one_height_with_transactions(&self.inner, &self.state_ref(), &[tx.raw().clone()]);
    }
}
