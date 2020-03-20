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

use anyhow::format_err;
use exonum_crypto::Hash;
use exonum_merkledb::{BinaryValue, Database, ObjectHash, TemporaryDB};
use pretty_assertions::{assert_eq, assert_ne};
use protobuf::Message;
use serde_json::json;

use std::{any::Any, panic};

use super::*;
use crate::{
    blockchain::{CallInBlock, Schema},
    helpers::Height,
};

fn make_panic<T: Send + 'static>(val: T) -> Box<dyn Any + Send> {
    panic::catch_unwind(panic::AssertUnwindSafe(|| panic!(val))).unwrap_err()
}

#[test]
fn execution_error_binary_value_round_trip() {
    let values = vec![
        (ErrorKind::Unexpected, "AAAA"),
        (ErrorKind::Core { code: 0 }, ""),
        (ErrorKind::Core { code: 0 }, "b"),
        (ErrorKind::Runtime { code: 1 }, "c"),
        (ErrorKind::Service { code: 18 }, "ddc"),
    ];

    for (kind, description) in values {
        let mut err = ExecutionError::new(kind, description.to_owned());
        let bytes = err.to_bytes();
        let err2 = ExecutionError::from_bytes(bytes.into()).unwrap();
        assert_eq!(err, err2);

        err.runtime_id = Some(1);
        let bytes = err.to_bytes();
        let err2 = ExecutionError::from_bytes(bytes.into()).unwrap();
        assert_eq!(err, err2);

        err.call_site = Some(CallSite::new(100, CallType::Constructor));
        let bytes = err.to_bytes();
        let err2 = ExecutionError::from_bytes(bytes.into()).unwrap();
        assert_eq!(err, err2);

        err.call_site = Some(CallSite::new(100, CallType::Resume));
        let bytes = err.to_bytes();
        let err2 = ExecutionError::from_bytes(bytes.into()).unwrap();
        assert_eq!(err, err2);

        err.call_site.as_mut().unwrap().call_type = CallType::Method {
            interface: "exonum.Configure".to_owned(),
            id: 1,
        };
        let bytes = err.to_bytes();
        let err2 = ExecutionError::from_bytes(bytes.into()).unwrap();
        assert_eq!(err, err2);
    }
}

#[test]
fn execution_error_binary_value_unexpected_with_code() {
    let bytes = {
        let mut inner = errors_proto::ExecutionError::default();
        inner.set_kind(errors_proto::ErrorKind::UNEXPECTED);
        inner.set_code(2);
        inner.write_to_bytes().unwrap()
    };

    assert_eq!(
        ExecutionError::from_bytes(bytes.into())
            .unwrap_err()
            .to_string(),
        "Error code for panic should be zero"
    )
}

#[allow(clippy::let_and_return)] // does not compile otherwise
fn error_hash(db: &TemporaryDB, err: &ExecutionError) -> Hash {
    let fork = db.fork();
    let mut schema = Schema::new(&fork);
    schema.save_error(Height(1), CallInBlock::transaction(1), err.to_owned());
    let error_hash = schema.call_errors_map(Height(1)).object_hash();
    error_hash
}

#[test]
fn execution_error_object_hash_description() {
    let db = TemporaryDB::new();
    let mut first_err = ExecutionError::new(ErrorKind::Service { code: 5 }, "foo".to_owned());
    let second_err = ExecutionError::new(ErrorKind::Service { code: 5 }, "foo bar".to_owned());
    assert_eq!(error_hash(&db, &first_err), error_hash(&db, &second_err));

    let second_err = ExecutionError::new(ErrorKind::Service { code: 6 }, "foo".to_owned());
    assert_ne!(error_hash(&db, &first_err), error_hash(&db, &second_err));

    let mut second_err = first_err.clone();
    second_err.runtime_id = Some(0);
    assert_ne!(error_hash(&db, &first_err), error_hash(&db, &second_err));
    first_err.runtime_id = Some(0);
    assert_eq!(error_hash(&db, &first_err), error_hash(&db, &second_err));
    first_err.runtime_id = Some(1);
    assert_ne!(error_hash(&db, &first_err), error_hash(&db, &second_err));

    let mut second_err = first_err.clone();
    second_err.call_site = Some(CallSite::new(100, CallType::Constructor));
    assert_ne!(error_hash(&db, &first_err), error_hash(&db, &second_err));

    first_err.call_site = Some(CallSite::new(100, CallType::Constructor));
    assert_eq!(error_hash(&db, &first_err), error_hash(&db, &second_err));

    second_err.call_site = Some(CallSite::new(101, CallType::Constructor));
    assert_ne!(error_hash(&db, &first_err), error_hash(&db, &second_err));

    second_err.call_site = Some(CallSite::new(100, CallType::AfterTransactions));
    assert_ne!(error_hash(&db, &first_err), error_hash(&db, &second_err));

    second_err.call_site = Some(CallSite::new(
        100,
        CallType::Method {
            interface: String::new(),
            id: 0,
        },
    ));
    assert_ne!(error_hash(&db, &first_err), error_hash(&db, &second_err));

    first_err.call_site = Some(CallSite::new(
        100,
        CallType::Method {
            interface: String::new(),
            id: 0,
        },
    ));
    assert_eq!(error_hash(&db, &first_err), error_hash(&db, &second_err));

    second_err.call_site = Some(CallSite::new(
        100,
        CallType::Method {
            interface: String::new(),
            id: 1,
        },
    ));
    assert_ne!(error_hash(&db, &first_err), error_hash(&db, &second_err));

    second_err.call_site = Some(CallSite::new(
        100,
        CallType::Method {
            interface: "foo".to_owned(),
            id: 0,
        },
    ));
    assert_ne!(error_hash(&db, &first_err), error_hash(&db, &second_err));
}

#[test]
fn execution_error_display() {
    let mut err = ExecutionError {
        kind: ErrorKind::Service { code: 3 },
        description: String::new(),
        runtime_id: Some(1),
        call_site: Some(CallSite::new(100, CallType::Constructor)),
    };
    let err_string = err.to_string();
    assert!(err_string.contains("Execution error with code `service:3`"));
    assert!(err_string.contains("in constructor of service 100"));
    assert!(!err_string.ends_with(": ")); // Empty description should not be output

    err.description = "Error description!".to_owned();
    assert!(err.to_string().ends_with(": Error description!"));

    err.call_site = Some(CallSite::new(
        200,
        CallType::Method {
            interface: "exonum.Configure".to_owned(),
            id: 0,
        },
    ));
    assert!(err
        .to_string()
        .contains("in exonum.Configure::(method 0) of service 200"));

    err.call_site = Some(CallSite::new(
        300,
        CallType::Method {
            interface: String::new(),
            id: 2,
        },
    ));
    assert!(err.to_string().contains("in method 2 of service 300"));

    err.call_site = None;
    assert!(err.to_string().contains("in Java runtime"));
}

#[test]
fn execution_result_serde_presentation() {
    let result = ExecutionStatus(Ok(()));
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({ "type": "success" })
    );

    let result = ExecutionStatus(Err(ExecutionError {
        kind: ErrorKind::Unexpected,
        description: "Some error".to_owned(),
        runtime_id: None,
        call_site: None,
    }));
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({
            "type": "unexpected_error",
            "description": "Some error",
        })
    );

    let result = ExecutionStatus(Err(ExecutionError {
        kind: ErrorKind::Service { code: 3 },
        description: String::new(),
        runtime_id: Some(1),
        call_site: Some(CallSite::new(100, CallType::Constructor)),
    }));
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({
            "type": "service_error",
            "code": 3,
            "runtime_id": 1,
            "call_site": {
                "instance_id": 100,
                "call_type": "constructor",
            }
        })
    );

    let result = ExecutionStatus(Err(ExecutionError {
        kind: ErrorKind::Service { code: 3 },
        description: String::new(),
        runtime_id: Some(1),
        call_site: Some(CallSite::new(100, CallType::Resume)),
    }));
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({
            "type": "service_error",
            "code": 3,
            "runtime_id": 1,
            "call_site": {
                "instance_id": 100,
                "call_type": "resume",
            }
        })
    );

    let result = ExecutionStatus(Err(ExecutionError {
        kind: ErrorKind::Core { code: 8 },
        description: "!".to_owned(),
        runtime_id: Some(0),
        call_site: Some(CallSite::new(
            100,
            CallType::Method {
                interface: "exonum.Configure".to_owned(),
                id: 1,
            },
        )),
    }));
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({
            "type": "core_error",
            "description": "!",
            "code": 8,
            "runtime_id": 0,
            "call_site": {
                "instance_id": 100,
                "call_type": "method",
                "interface": "exonum.Configure",
                "method_id": 1,
            }
        })
    );
}

#[test]
fn execution_result_serde_roundtrip() {
    let values = vec![
        Err((ErrorKind::Unexpected, "AAAA")),
        Err((ErrorKind::Core { code: 0 }, "")),
        Err((ErrorKind::Core { code: 0 }, "b")),
        Err((ErrorKind::Runtime { code: 1 }, "c")),
        Err((ErrorKind::Service { code: 18 }, "ddc")),
        Ok(()),
    ];

    for value in values {
        let mut res = ExecutionStatus(
            value.map_err(|(kind, description)| ExecutionError::new(kind, description.to_owned())),
        );
        let json = serde_json::to_string_pretty(&res).unwrap();
        let res2 = serde_json::from_str(&json).unwrap();
        assert_eq!(res, res2);

        if let Err(err) = res.0.as_mut() {
            err.runtime_id = Some(1);
            let json = serde_json::to_string_pretty(&res).unwrap();
            let res2 = serde_json::from_str(&json).unwrap();
            assert_eq!(res, res2);
        }

        if let Err(err) = res.0.as_mut() {
            err.call_site = Some(CallSite::new(1_000, CallType::AfterTransactions));
            let json = serde_json::to_string_pretty(&res).unwrap();
            let res2 = serde_json::from_str(&json).unwrap();
            assert_eq!(res, res2);
        }

        if let Err(err) = res.0.as_mut() {
            err.call_site = Some(CallSite::new(
                1_000,
                CallType::Method {
                    interface: "exonum.Configure".to_owned(),
                    id: 1,
                },
            ));
            let json = serde_json::to_string_pretty(&res).unwrap();
            let res2 = serde_json::from_str(&json).unwrap();
            assert_eq!(res, res2);
        }
    }
}

#[test]
fn str_panic() {
    let static_str = "Static string (&str)";
    let panic = make_panic(static_str);
    assert_eq!(ExecutionError::from_panic(panic).description, static_str);
}

#[test]
fn string_panic() {
    let string = "Owned string (String)".to_owned();
    let panic = make_panic(string.clone());
    assert_eq!(ExecutionError::from_panic(panic).description, string);
}

#[test]
fn box_error_panic() {
    let error: Box<dyn std::error::Error + Send> = Box::new("e".parse::<i32>().unwrap_err());
    let description = error.to_string();
    let panic = make_panic(error);
    assert_eq!(ExecutionError::from_panic(panic).description, description);
}

#[test]
fn failure_panic() {
    let error = format_err!("Failure panic");
    let description = error.to_string();
    let panic = make_panic(error);
    assert_eq!(ExecutionError::from_panic(panic).description, description);
}

#[test]
fn unknown_panic() {
    let panic = make_panic(1);
    assert_eq!(ExecutionError::from_panic(panic).description, "");
}
