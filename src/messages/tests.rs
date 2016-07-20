// #[test]
// fn test_connect() {
//     use std::str::FromStr;

//     let socket_address = SocketAddr::from_str("18.34.3.4:7777").unwrap();
//     let time = ::time::get_time();
//     let (public_key, secret_key) = super::crypto::gen_keypair();

//     // write
//     let connect = Connect::new(socket_address.clone(), time,
//                                &public_key, &secret_key);
//     // read
//     assert_eq!(connect.addr(), socket_address);
//     assert_eq!(connect.time(), time);
//     assert!(connect.verify());
// }

// #[test]
// fn test_propose() {
//     let height = 123_123_123;
//     let round = 321_321_312;
//     let time = ::time::get_time();
//     let prev_hash = super::crypto::hash(&[1, 2, 3]);
//     let (public_key, secret_key) = super::crypto::gen_keypair();

//     // write
//     let propose = Propose::new(height, round, time, &prev_hash,
//                                &public_key, &secret_key);
//     // read
//     assert_eq!(propose.height(), height);
//     assert_eq!(propose.round(), round);
//     assert_eq!(propose.time(), time);
//     assert_eq!(propose.prev_hash(), &prev_hash);
//     assert!(propose.verify());
// }

// #[test]
// fn test_prevote() {
//     let height = 123_123_123;
//     let round = 321_321_312;
//     let hash = super::crypto::hash(&[1, 2, 3]);
//     let (public_key, secret_key) = super::crypto::gen_keypair();

//     // write
//     let prevote = Prevote::new(height, round, &hash, &public_key, &secret_key);
//     // read
//     assert_eq!(prevote.height(), height);
//     assert_eq!(prevote.round(), round);
//     assert_eq!(prevote.hash(), &hash);
//     assert!(prevote.verify());
// }

// #[test]
// fn test_precommit() {
//     let height = 123_123_123;
//     let round = 321_321_312;
//     let hash = super::crypto::hash(&[1, 2, 3]);
//     let (public_key, secret_key) = super::crypto::gen_keypair();

//     // write
//     let precommit = Precommit::new(height, round, &hash,
//                                    &public_key, &secret_key);
//     // read
//     assert_eq!(precommit.height(), height);
//     assert_eq!(precommit.round(), round);
//     assert_eq!(precommit.hash(), &hash);
//     assert!(precommit.verify());
// }

// #[test]
// fn test_commit() {
//     let height = 123_123_123;
//     let hash = super::crypto::hash(&[1, 2, 3]);
//     let (public_key, secret_key) = super::crypto::gen_keypair();

//     // write
//     let commit = Commit::new(height, &hash, &public_key, &secret_key);
//     // read
//     assert_eq!(commit.height(), height);
//     assert_eq!(commit.hash(), &hash);
//     assert!(commit.verify());
// }

