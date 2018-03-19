// Copyright 2018 The Exonum Team
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

use synstructure::Structure;

use base::base_derive;
use message::message_derive;

#[test]
#[should_panic(expected = "attribute not specified")]
fn message_derivation_no_payload() {
    let input = parse_quote!(
        #[derive(Clone, Message)]
        pub struct Message(RawMessage);
    );
    let s = Structure::new(&input);
    message_derive(s);
}

#[test]
#[should_panic(expected = "attribute specified multiple times")]
fn message_derivation_multiple_payloads() {
    let input = parse_quote!(
        #[derive(Clone, Message)]
        #[exonum(payload = "Transactions")]
        #[exonum(payload = "OtherTransactions")]
        pub struct Message(RawMessage);
    );
    let s = Structure::new(&input);
    message_derive(s);
}

#[test]
#[should_panic(expected = "invalid type for `Message` derivation")]
fn message_derivation_invalid_struct() {
    let input = parse_quote!(
        #[derive(Clone, Message)]
        #[exonum(payload = "Transactions")]
        pub struct Message(RawMessage, u64);
    );
    let s = Structure::new(&input);
    message_derive(s);
}

#[test]
#[should_panic(expected = "invalid type for `Message` derivation")]
fn message_derivation_named_fields_struct() {
    let input = parse_quote!(
        #[derive(Clone, Message)]
        #[exonum(payload = "Transactions")]
        pub struct Message {
            inner: RawMessage,
        }
    );
    let s = Structure::new(&input);
    message_derive(s);
}

#[test]
#[should_panic(expected = "invalid type for `Message` derivation")]
fn message_derivation_empty_struct() {
    let input = parse_quote!(
        #[derive(Clone, Message)]
        #[exonum(payload = "Transactions")]
        pub struct Message;
    );
    let s = Structure::new(&input);
    message_derive(s);
}

#[test]
#[should_panic(expected = "invalid type for `Message` derivation")]
fn message_derivation_enum() {
    let input = parse_quote!(
        #[derive(Clone, Message)]
        #[exonum(payload = "Transactions")]
        pub enum Message {
            Foo,
            Bar,
        }
    );
    let s = Structure::new(&input);
    message_derive(s);
}

#[test]
#[should_panic(expected = "malformed `#[exonum(payload)]` attribute")]
fn message_derivation_incorrect_payload_spec() {
    let input = parse_quote!(
        #[derive(Clone, Message)]
        #[exonum(payload = "2 + 2 = 5")]
        pub struct Message(RawMessage);
    );
    let s = Structure::new(&input);
    message_derive(s);
}

#[test]
#[should_panic(expected = "missing `#[exonum(service_id)]` attribute")]
fn message_set_derivation_no_service_id() {
    let input = parse_quote!(
        #[derive(Clone, MessageSet)]
        pub enum Transactions<'a> {
            CreateWallet {
                public_key: &'a PublicKey,
                name: &'a str,
            },

            Transfer {
                from: &'a PublicKey,
                to: &'a PublicKey,
                amount: u64,
            },
        }
    );
    let s = Structure::new(&input);
    base_derive(s);
}

#[test]
#[should_panic(expected = "duplicate `#[exonum(service_id)]` attribute")]
fn message_set_derivation_duplicate_service_id() {
    let input = parse_quote!(
        #[derive(Clone, MessageSet)]
        #[exonum(service_id = "10", service_id = "SERVICE_ID")]
        pub enum Transactions<'a> {
            CreateWallet {
                public_key: &'a PublicKey,
                name: &'a str,
            },

            Transfer {
                from: &'a PublicKey,
                to: &'a PublicKey,
                amount: u64,
            },
        }
    );
    let s = Structure::new(&input);
    base_derive(s);
}

#[test]
#[should_panic(expected = "malformed `#[exonum(service_id)]` attribute")]
fn message_set_derivation_malformed_service_id() {
    let input = parse_quote!(
        #[derive(Clone, MessageSet)]
        #[exonum(service_id = ":100:")]
        pub enum Transactions<'a> {
            CreateWallet {
                public_key: &'a PublicKey,
                name: &'a str,
            },

            Transfer {
                from: &'a PublicKey,
                to: &'a PublicKey,
                amount: u64,
            },
        }
    );
    let s = Structure::new(&input);
    base_derive(s);
}

#[test]
#[should_panic(expected = "unsupported generic params in messages declaration")]
fn message_set_derivation_type_params() {
    let input = parse_quote!(
        #[derive(Clone, MessageSet)]
        #[exonum(service_id = "10")]
        pub enum Transactions<'a, T> {
            CreateWallet {
                public_key: &'a T,
                name: &'a str,
            },

            Transfer {
                from: &'a T,
                to: &'a T,
                amount: u64,
            },
        }
    );
    let s = Structure::new(&input);
    base_derive(s);
}

#[test]
#[should_panic(expected = "unsupported generic params in messages declaration")]
fn message_set_derivation_multiple_lifetimes() {
    let input = parse_quote!(
        #[derive(Clone, MessageSet)]
        #[exonum(service_id = "10")]
        pub enum Transactions<'a, 'b> {
            CreateWallet {
                public_key: &'a PublicKey,
                name: &'a str,
            },

            Transfer {
                from: &'b PublicKey,
                to: &'a PublicKey,
                amount: u64,
            },
        }
    );
    let s = Structure::new(&input);
    base_derive(s);
}
