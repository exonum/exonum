pub use sodiumoxide::crypto::sign::ed25519::{PublicKey, SecretKey, Seed, Signature,
                                             sign_detached as sign, verify_detached as verify,
                                             gen_keypair,
                                             keypair_from_seed as gen_keypair_from_seed,
                                             PUBLICKEYBYTES as PUBLIC_KEY_LENGTH,
                                             SECRETKEYBYTES as SECRET_KEY_LENGTH,
                                             SIGNATUREBYTES as SIGNATURE_LENGTH,
                                             SEEDBYTES as SEED_LENGTH};

pub use sodiumoxide::crypto::hash::sha256::{hash, Digest as Hash, DIGESTBYTES as HASH_SIZE};
