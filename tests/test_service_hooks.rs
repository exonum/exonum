#[macro_use]
extern crate exonum;
extern crate exonum_testkit;
extern crate serde;
extern crate serde_json;

use exonum::crypto::Signature;
use exonum::messages::Message;
use exonum::helpers::Height;
use exonum_testkit::TestKitBuilder;

mod hooks {
    //! A special service which generates transactions on `handle_commit` events.

    use serde_json::{to_value, Value};

    use exonum::blockchain::{Service, ServiceContext, Transaction};
    use exonum::messages::{FromRaw, RawTransaction};
    use exonum::storage::Fork;
    use exonum::crypto::Signature;
    use exonum::encoding;
    use exonum::helpers::Height;

    const SERVICE_ID: u16 = 1;
    const TX_AFTER_COMMIT_ID: u16 = 1;

    message! {
        struct TxAfterCommit {
            const TYPE = SERVICE_ID;
            const ID = TX_AFTER_COMMIT_ID;
            const SIZE = 8;

            field height: Height [0 => 8]
        }
    }

    impl Transaction for TxAfterCommit {
        fn verify(&self) -> bool {
            true
        }

        fn execute(&self, _fork: &mut Fork) {}

        fn info(&self) -> Value {
            to_value(self).unwrap()
        }
    }

    pub struct HandleCommitService;

    impl Service for HandleCommitService {
        fn service_name(&self) -> &'static str {
            "handle_commit"
        }

        fn service_id(&self) -> u16 {
            SERVICE_ID
        }

        fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
            let tx: Box<Transaction> = match raw.message_type() {
                TX_AFTER_COMMIT_ID => Box::new(TxAfterCommit::from_raw(raw)?),
                _ => {
                    return Err(encoding::Error::IncorrectMessageType {
                        message_type: raw.message_type(),
                    });
                }
            };
            Ok(tx)
        }

        fn handle_commit(&self, context: &ServiceContext) {
            let tx = TxAfterCommit::new_with_signature(context.height(), &Signature::zero());
            context.transaction_sender().send(Box::new(tx)).unwrap();
        }
    }
}

// HACK: Silent "dead_code" warning.
pub use hooks::{HandleCommitService, TxAfterCommit};

#[test]
fn test_handle_commit() {
    let mut testkit = TestKitBuilder::validator()
        .with_service(HandleCommitService)
        .create();
    // Check that `handle_commit` invoked on the correct height.
    for i in 1..5 {
        testkit.create_block();
        let tx = TxAfterCommit::new_with_signature(Height(i), &Signature::zero());
        assert!(testkit.mempool().contains_key(&tx.hash()));
    }
}
