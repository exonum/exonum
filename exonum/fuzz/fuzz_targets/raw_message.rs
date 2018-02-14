#![no_main]
#[macro_use] extern crate libfuzzer_sys;
extern crate exonum;

use exonum::messages::RawMessage;

fn fuzz_target(data: &[u8]) {
    let msg = RawMessage::from_vec(data.to_vec());

    let _ = msg.version();
    let _ = msg.network_id();
    let _ = msg.service_id();
    let _ = msg.message_type();
    let _ = msg.body();
    let _ = msg.signature();
}

fuzz_target!(|data| {
    fuzz_target(data);
});
