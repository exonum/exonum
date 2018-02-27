#[cfg(test)]
#[macro_use]
extern crate log;
extern crate exonum;
extern crate sandbox;
extern crate cryptocurrency;
extern crate iron;
extern crate router;
extern crate serde;
#[cfg(test)]
#[macro_use]
extern crate serde_derive;
extern crate rand;

#[cfg(test)]
mod tests {
    extern crate iron_test;

    use rand::{thread_rng, Rng};
    use router::Router;
    use iron::Headers;
    use iron::status::Status;
    use iron::prelude::*;
    use iron::headers::ContentType;
    use serde::Serialize;

    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::path::Path;
    use std::fs::File;
    use std::io::{Read, Error};

    use exonum::encoding::serialize::json::reexport as serde_json;
    use exonum::node::TransactionSend;
    use exonum::crypto::{Seed, Hash, PublicKey, gen_keypair, gen_keypair_from_seed};
    use exonum::blockchain::{Service, Transaction};
    use exonum::events::error;
    use exonum::messages::{Message, RawMessage};
    use exonum::api::Api;
    use exonum::helpers::init_logger;
    use cryptocurrency::{CurrencyService, CurrencyTx, TxCreateWallet, TxIssue, TxTransfer};
    use cryptocurrency::api::CryptocurrencyApi;
    use sandbox::sandbox::{sandbox_with_services, Sandbox};
    use sandbox::sandbox_tests_helper::{add_one_height_with_transactions, SandboxState};

    #[test]
    fn test_sandbox_validators_list() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let actual_validators = serde_json::to_value(&sandbox.sandbox.validators()).unwrap();
        info!("{}", serde_json::to_string(&actual_validators).unwrap());
        let expected_validators = from_file("test_data/validators.json");
        assert_eq!(actual_validators, expected_validators, "validators.json");
    }

    #[test]
    fn test_create_wallet_correct_post() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let mut rng = thread_rng();

        let generator_create = move |ind| {
            if ind == 0 {
                let (p, s) = gen_keypair_from_seed(&Seed::new([255; 32]));
                return TxCreateWallet::new(
                    &p,
                    "babd, Юникод еще работает",
                    &s,
                ).into();
            }
            let (p, s) = gen_keypair();
            let string_len = rng.gen_range(20u8, 255u8);
            let name: String = rng.gen_ascii_chars().take(string_len as usize).collect();
            TxCreateWallet::new(&p, &name, &s).into()
        };

        test_txs_post(&sandbox, generator_create);
    }

    #[test]
    fn test_issue_correct_post() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let mut rng = thread_rng();

        let generator_issue = move |_| {
            let (p, s) = gen_keypair();
            let amount = rng.next_u64();
            let seed = rng.next_u64();
            TxIssue::new(&p, amount, seed, &s).into()
        };

        test_txs_post(&sandbox, generator_issue);
    }

    #[test]
    fn test_transfer_correct_post() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let mut rng = thread_rng();

        let generator_transfer = move |_| {
            let (p_from, s) = gen_keypair();
            let (p_to, _) = gen_keypair();
            let amount = rng.next_u64();
            let seed = rng.next_u64();
            TxTransfer::new(&p_from, &p_to, amount, seed, &s).into()
        };

        test_txs_post(&sandbox, generator_transfer);
    }

    #[test]
    fn test_create_incorrect_signature() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let (p1, _) = gen_keypair();
        let (_, s2) = gen_keypair();

        let invalid_tx = TxCreateWallet::new(&p1, "Incorrect_signature", &s2);
        let invalid_tx: CurrencyTx = invalid_tx.into();
        let resp = sandbox.post_transaction(invalid_tx);
        assert_response_status(resp, Status::Conflict, "Unable to verify transaction");
    }

    #[test]
    fn test_issue_incorrect_signature() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let (p1, _) = gen_keypair();
        let (_, s2) = gen_keypair();

        let invalid_tx: CurrencyTx = TxIssue::new(&p1, 2600, 7000, &s2).into();
        let resp = sandbox.post_transaction(invalid_tx);
        assert_response_status(resp, Status::Conflict, "Unable to verify transaction");
    }

    #[test]
    fn test_transfer_incorrect_signature() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let (p1, s1) = gen_keypair();
        let (p2, s2) = gen_keypair();

        let invalid_tx: CurrencyTx = TxTransfer::new(&p1, &p2, 2000, 4242342, &s2).into();
        let resp = sandbox.post_transaction(invalid_tx);
        assert_response_status(resp, Status::Conflict, "Unable to verify transaction");

        //identiacal sender == receiver are also marked as invalid transaction and
        // not shoved down the event loop
        // Transaction::verify() for CurrenctyTx
        let invalid_tx: CurrencyTx = TxTransfer::new(&p1, &p1, 2000, 4242342, &s1).into();
        let resp = sandbox.post_transaction(invalid_tx);
        assert_response_status(resp, Status::Conflict, "Unable to verify transaction");
    }

    #[test]
    fn test_message_id_from_other_type() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let tx_malformed_mes_id = from_file("test_data/message_id_from_other.json");
        let resp = sandbox.post_transaction(tx_malformed_mes_id);
        assert_response_status(
            resp,
            Status::Conflict,
            "data did not match any variant of untagged enum CurrencyTx",
        );
    }

    #[test]
    fn test_invalid_message_id() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let tx_malformed_mes_id = from_file("test_data/invalid_message_id.json");
        let resp = sandbox.post_transaction(tx_malformed_mes_id);
        assert_response_status(
            resp,
            Status::Conflict,
            "data did not match any variant of untagged enum CurrencyTx",
        );
    }

    #[test]
    fn test_invalid_service_id() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let tx_malformed_mes_id = from_file("test_data/invalid_service_id.json");
        let resp = sandbox.post_transaction(tx_malformed_mes_id);
        assert_response_status(
            resp,
            Status::Conflict,
            "data did not match any variant of untagged enum CurrencyTx",
        );
    }

    #[test]
    fn test_incorrect_wallet_info_query() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();

        // it's 1 byte and 2 hex symbols shorter
        let pubkey_malformed_str = "cec750b8a1723960c9708a4fe11e49d90ac4592d0bd99d9b3757f8a0f517a3";
        let resp = sandbox.request_wallet_info_str(pubkey_malformed_str);
        assert_response_status(resp, Status::Conflict, "InvalidStringLength");
    }

    #[test]
    fn test_query_absent_wallet() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let (p1, s1) = gen_keypair_from_seed(&Seed::new([11; 32]));
        let (p2, s2) = gen_keypair_from_seed(&Seed::new([12; 32]));
        let (p3, s3) = gen_keypair_from_seed(&Seed::new([13; 32]));
        let (p4, s4) = gen_keypair_from_seed(&Seed::new([14; 32]));
        let (p5, s5) = gen_keypair_from_seed(&Seed::new([15; 32]));
        let (p6, s6) = gen_keypair_from_seed(&Seed::new([16; 32]));
        let txs: Vec<CurrencyTx> =
            vec![
                TxCreateWallet::new(&p1, "Jane Doe", &s1).into(),
                TxCreateWallet::new(&p2, "Dillinger Escape Plan", &s2).into(),
                TxCreateWallet::new(&p3, "wallet3", &s3).into(),
                TxCreateWallet::new(&p4, "walet4", &s4).into(),
                TxCreateWallet::new(&p5, "wallet5", &s5).into(),
                TxCreateWallet::new(&p6, "wallet6", &s6).into(),
            ];
        txs.iter()
            .inspect(|tx| { sandbox.post_transaction((*tx).clone()).unwrap(); })
            .collect::<Vec<_>>();
        sandbox.commit();
        let (p_absent, _) = gen_keypair_from_seed(&Seed::new([27; 32]));
        let resp_absent_wallet = sandbox.request_wallet_info(&p_absent).unwrap();
        let actual_body = response_body(resp_absent_wallet);
        let expected_body = from_file("test_data/response_absent_wallet.json");
        assert_eq!(actual_body, expected_body, "response_absent_wallet.json");
    }

    #[test]
    fn test_query_wallet_with_history() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let (p1, s1) = gen_keypair_from_seed(&Seed::new([11; 32]));
        let (p2, s2) = gen_keypair_from_seed(&Seed::new([12; 32]));
        let (p3, s3) = gen_keypair_from_seed(&Seed::new([13; 32]));
        let (p4, s4) = gen_keypair_from_seed(&Seed::new([14; 32]));
        let (p5, s5) = gen_keypair_from_seed(&Seed::new([15; 32]));
        let (p6, s6) = gen_keypair_from_seed(&Seed::new([16; 32]));
        let txs: Vec<CurrencyTx> =
            vec![
                TxCreateWallet::new(&p1, "Jane Doe", &s1).into(),
                TxCreateWallet::new(&p2, "Dillinger Escape Plan", &s2).into(),
                TxCreateWallet::new(&p3, "wallet3", &s3).into(),
                TxCreateWallet::new(&p4, "walet4", &s4).into(),
                TxCreateWallet::new(&p5, "wallet5", &s5).into(),
                TxCreateWallet::new(&p6, "wallet6", &s6).into(),
            ];
        txs.iter()
            .inspect(|tx| { sandbox.post_transaction((*tx).clone()).unwrap(); })
            .collect::<Vec<_>>();
        sandbox.commit();

        sandbox
            .post_transaction(CurrencyTx::from(TxIssue::new(&p1, 6000, 1000, &s1)))
            .unwrap();
        sandbox.commit();

        sandbox
            .post_transaction(CurrencyTx::from(TxTransfer::new(&p1, &p2, 3000, 2000, &s1)))
            .unwrap();
        sandbox.commit();

        sandbox
            .post_transaction(CurrencyTx::from(TxTransfer::new(&p2, &p1, 1000, 3000, &s2)))
            .unwrap();
        sandbox.commit();

        let resp_wallet1 = sandbox.request_wallet_info(&p1).unwrap();
        let body1 = response_body(resp_wallet1);
        let expected_body1 = from_file("test_data/wallet1_query.json");
        assert_eq!(body1, expected_body1, "wallet1_query.json");

        let resp_wallet2 = sandbox.request_wallet_info(&p2).unwrap();
        let body2 = response_body(resp_wallet2);
        let expected_body2 = from_file("test_data/wallet2_query.json");
        assert_eq!(body2, expected_body2, "wallet2_query.json");
    }

    #[test]
    fn test_commit_txs_no_state_change() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let (p1, s1) = gen_keypair_from_seed(&Seed::new([11; 32]));
        let (p2, s2) = gen_keypair_from_seed(&Seed::new([12; 32]));
        let (p3, s3) = gen_keypair_from_seed(&Seed::new([13; 32]));
        let (p4, s4) = gen_keypair_from_seed(&Seed::new([14; 32]));
        let (p5, s5) = gen_keypair_from_seed(&Seed::new([15; 32]));
        let (p6, s6) = gen_keypair_from_seed(&Seed::new([16; 32]));
        let txs: Vec<CurrencyTx> =
            vec![
                TxCreateWallet::new(&p1, "Jane Doe", &s1).into(),
                TxCreateWallet::new(&p2, "Dillinger Escape Plan", &s2).into(),
                TxCreateWallet::new(&p3, "wallet3", &s3).into(),
                TxCreateWallet::new(&p4, "walet4", &s4).into(),
                TxCreateWallet::new(&p5, "wallet5", &s5).into(),
                TxCreateWallet::new(&p6, "wallet6", &s6).into(),
            ];
        txs.iter()
            .inspect(|tx| { sandbox.post_transaction((*tx).clone()).unwrap(); })
            .collect::<Vec<_>>();
        sandbox.commit();

        sandbox
            .post_transaction(CurrencyTx::from(TxIssue::new(&p1, 6000, 1000, &s1)))
            .unwrap();
        sandbox.commit();

        sandbox
            .post_transaction(CurrencyTx::from(TxTransfer::new(&p1, &p2, 3000, 2000, &s1)))
            .unwrap();
        sandbox.commit();

        sandbox
            .post_transaction(CurrencyTx::from(TxTransfer::new(&p2, &p1, 1000, 3000, &s2)))
            .unwrap();
        sandbox.commit();

        let resp_wallet1 = sandbox.request_wallet_info(&p1).unwrap();
        let body1 = response_body(resp_wallet1);
        let expected_body1 = from_file("test_data/wallet1_query.json");
        assert_eq!(body1, expected_body1, "wallet1_query.json");

        sandbox
            .post_transaction(CurrencyTx::from(TxCreateWallet::new(
                &p1,
                "Change name of existing \
                                                                    wallet",
                &s1,
            )))
            .unwrap();
        sandbox.commit();

        let resp_wallet1 = sandbox.request_wallet_info(&p1).unwrap();
        let body1 = response_body(resp_wallet1);
        let expected_body1 = from_file("test_data/tx_create_wallet_false_execution_status.json");
        assert_eq!(
            body1,
            expected_body1,
            "tx_create_wallet_false_execution_status.json"
        );

        let (p_absent, s_absent) = gen_keypair_from_seed(&Seed::new([27; 32]));
        sandbox
            .post_transaction(CurrencyTx::from(
                TxIssue::new(&p_absent, 6000, 329832, &s_absent),
            ))
            .unwrap();
        sandbox.commit();

        let resp_wallet1 = sandbox.request_wallet_info(&p1).unwrap();
        let body1 = response_body(resp_wallet1);
        let expected_body1 = from_file("test_data/no_state_change2.json");
        assert_eq!(body1, expected_body1, "no_state_change2.json");

        sandbox
            .post_transaction(CurrencyTx::from(
                TxTransfer::new(&p1, &p2, 1_000_000, 329832, &s1),
            ))
            .unwrap();
        sandbox.commit();

        let resp_wallet1 = sandbox.request_wallet_info(&p1).unwrap();
        let body1 = response_body(resp_wallet1);
        let expected_body1 = from_file("test_data/commit_new_transfer_not_sufficient_funds.json");
        assert_eq!(
            body1,
            expected_body1,
            "commit_new_transfer_not_sufficient_funds.json"
        );
    }

    #[test]
    fn test_commit_duplicate_txs() {
        let _ = init_logger();
        let sandbox = CurrencySandbox::new();
        let (p1, s1) = gen_keypair_from_seed(&Seed::new([11; 32]));
        let (p2, s2) = gen_keypair_from_seed(&Seed::new([12; 32]));
        let (p3, s3) = gen_keypair_from_seed(&Seed::new([13; 32]));
        let (p4, s4) = gen_keypair_from_seed(&Seed::new([14; 32]));
        let (p5, s5) = gen_keypair_from_seed(&Seed::new([15; 32]));
        let (p6, s6) = gen_keypair_from_seed(&Seed::new([16; 32]));
        let txs: Vec<CurrencyTx> =
            vec![
                TxCreateWallet::new(&p1, "Jane Doe", &s1).into(),
                TxCreateWallet::new(&p2, "Dillinger Escape Plan", &s2).into(),
                TxCreateWallet::new(&p3, "wallet3", &s3).into(),
                TxCreateWallet::new(&p4, "walet4", &s4).into(),
                TxCreateWallet::new(&p5, "wallet5", &s5).into(),
                TxCreateWallet::new(&p6, "wallet6", &s6).into(),
            ];
        txs.iter()
            .inspect(|tx| { sandbox.post_transaction((*tx).clone()).unwrap(); })
            .collect::<Vec<_>>();
        sandbox.commit();

        sandbox
            .post_transaction(CurrencyTx::from(TxIssue::new(&p1, 6000, 1000, &s1)))
            .unwrap();
        sandbox.commit();

        let resp_wallet1 = sandbox.request_wallet_info(&p1).unwrap();
        let body1 = response_body(resp_wallet1);
        let expected_body1 = from_file("test_data/wallet1_query1.json");
        assert_eq!(body1, expected_body1, "wallet1_query1.json");

        sandbox
            .post_transaction(CurrencyTx::from(TxIssue::new(&p1, 6000, 1000, &s1)))
            .unwrap();
        sandbox.commit();

        let resp_wallet1 = sandbox.request_wallet_info(&p1).unwrap();
        let body1 = response_body(resp_wallet1);
        let expected_body1 = from_file("test_data/no_txs_committed_no_state_change.json");
        assert_eq!(
            body1,
            expected_body1,
            "no_txs_committed_no_state_change.json"
        );
    }



    fn assert_response_status(
        response: IronResult<Response>,
        expected_status: Status,
        expected_message: &str,
    ) {
        assert!(response.is_err());
        match response {
            Err(iron_error) => {
                let resp = iron_error.response;
                debug!("Error response: {}", resp);
                assert_eq!(resp.status, Some(expected_status));
                let body = response_body_str(resp).unwrap();
                assert!(&body.contains(expected_message));
            }
            _ => unreachable!(),
        }
    }

    fn test_txs_post<F>(sandbox: &CurrencySandbox, generator: F)
    where
        F: FnMut(usize) -> CurrencyTx,
    {
        (0..50)
            .map(generator)
            .inspect(|tx| {
                let expected_tx_hash = tx.hash();
                let resp = sandbox.post_transaction(tx.clone()).unwrap();
                let body = response_body(resp);
                let tx_response_res = serde_json::from_value::<TxResponse>(body).unwrap();
                assert_eq!(expected_tx_hash, tx_response_res.tx_hash);
            })
            .collect::<Vec<CurrencyTx>>();
    }

    fn response_body(response: Response) -> serde_json::Value {
        if let Some(body_string) = response_body_str(response) {
            serde_json::from_str(&body_string).unwrap()
        } else {
            serde_json::Value::Null
        }
    }

    fn response_body_str(response: Response) -> Option<String> {
        response.body.map(|mut body| {
            let mut buf = Vec::new();
            body.write_body(&mut buf).unwrap();
            let s = String::from_utf8(buf).unwrap();
            debug!("Received response body:'{}'", &s);
            s
        })
    }

    fn request_get<A: AsRef<str>>(route: A, router: &Router) -> IronResult<Response> {
        let url = format!("http://127.0.0.1:8000/{}", route.as_ref());
        info!("GET request:'{}'", url);
        iron_test::request::get(&url, Headers::new(), router)
    }

    fn request_post_str<B: AsRef<str>, A: AsRef<str>>(
        route: A,
        body: B,
        router: &Router,
    ) -> IronResult<Response> {
        let body_str = body.as_ref();
        let mut headers = Headers::new();
        headers.set(ContentType::json());
        let url = format!("http://127.0.0.1:8000/{}", route.as_ref());
        info!("POST request:'{}' with body '{}'", url, body_str);
        iron_test::request::post(&url, headers, body_str, router)
    }

    fn request_post_body<T: Serialize, A: AsRef<str>>(
        route: A,
        body: T,
        router: &Router,
    ) -> IronResult<Response> {
        let body_str: &str = &serde_json::to_string(&body).unwrap();
        request_post_str(route, body_str, router)
    }

    fn from_file<P: AsRef<Path>>(path: P) -> serde_json::Value {
        let mut file = File::open(path).unwrap();
        let mut s = String::new();
        file.read_to_string(&mut s).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[derive(Clone)]
    struct TestTxSender {
        transactions: Arc<Mutex<VecDeque<RawMessage>>>,
    }

    impl TransactionSend for TestTxSender {
        fn send(&self, tx: Box<Transaction>) -> Result<(), Error> {
            if tx.verify() {
                let rm = tx.raw().clone();
                self.transactions.lock().unwrap().push_back(rm);
                Ok(())
            } else {
                Err(error::other_error("Unable to verify transaction"))
            }
        }
    }

    struct CurrencySandbox {
        pub sandbox: Sandbox,
        pub state: SandboxState,
        pub transactions: Arc<Mutex<VecDeque<RawMessage>>>,
    }


    impl CurrencySandbox {
        fn new() -> CurrencySandbox {
            let services: Vec<Box<Service>> = vec![Box::new(CurrencyService::new())];
            let sandbox = sandbox_with_services(services);
            info!(
                "Sandbox validators list: {}",
                serde_json::to_string(&sandbox.validators()).unwrap()
            );
            let state = SandboxState::new();
            CurrencySandbox {
                sandbox,
                state,
                transactions: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        fn obtain_test_api(&self) -> Router {
            let channel = TestTxSender { transactions: self.transactions.clone() };
            let blockchain = self.sandbox.blockchain_ref().clone();
            let api = CryptocurrencyApi {
                channel,
                blockchain,
            };
            let mut router = Router::new();
            api.wire(&mut router);
            router
        }

        fn commit(&self) {
            let mut collected_transactions = self.transactions.lock().unwrap();
            let txs = collected_transactions.drain(..).collect::<Vec<_>>();
            debug!("Sandbox commits a sequence of {} transactions", txs.len());
            for elem in &txs {
                trace!("Message hash: {:?}", (*elem).hash());
                trace!("{:?}", CurrencyTx::from_raw((*elem).clone()));
            }
            add_one_height_with_transactions(&self.sandbox, &self.state, txs.iter());
        }

        fn request_wallet_info_str<A: AsRef<str>>(
            &self,
            public_key_str: A,
        ) -> IronResult<Response> {
            let api = self.obtain_test_api();
            let get_route = format!("/v1/wallets/info?pubkey={}", public_key_str.as_ref());
            request_get(get_route, &api)
        }

        fn request_wallet_info(&self, pulic_key: &PublicKey) -> IronResult<Response> {
            let pubkey_str = serde_json::to_string(&pulic_key).unwrap().replace("\"", "");
            self.request_wallet_info_str(pubkey_str)
        }

        fn post_transaction<T: Serialize>(&self, tx: T) -> IronResult<Response> {
            let api = self.obtain_test_api();
            let post_route = "/v1/wallets/transaction";
            request_post_body(post_route, tx, &api)
        }
    }

    #[derive(Deserialize)]
    struct TxResponse {
        tx_hash: Hash,
    }
}
