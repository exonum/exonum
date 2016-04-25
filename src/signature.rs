pub use sodiumoxide::crypto::sign::ed25519::{
    PublicKey, SecretKey, Seed, Signature,
    sign_detached as sign,
    verify_detached as verify,
    gen_keypair
};
