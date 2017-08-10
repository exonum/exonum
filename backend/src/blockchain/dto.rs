use std::time::SystemTime;

use exonum::crypto::{PublicKey, Hash};

use TIMESTAMPING_SERVICE_ID;

pub const TX_UPDATE_USER_ID: u16 = 0;
pub const TX_PAYMENT_ID: u16 = 1;
pub const TX_TIMESTAMP: u16 = 2;

encoding_struct! {
    /// Information about timestapming user.
    struct UserInfo {
        const SIZE = 56;

        /// Unique user identifier.
        field id:                       &str        [00 => 08]
        /// Public key of user.
        field public_key:               &PublicKey  [08 => 40]
        /// Encrypted secret key.
        field encrypted_secret_key:     Vec<u8>     [40 => 48]
        /// Additional metadata.
        field metadata:                 &str        [48 => 56]
    }
}

encoding_struct! {
    /// Information about payment.
    struct PaymentInfo {
        const SIZE = 24;

        /// Unique user identifier.
        field user_id:                  &str        [00 => 08]
        /// Total amount of available transactions.
        field total_amount:             u64         [08 => 16]
        /// Additional metadata.
        field metadata:                 &str        [16 => 24]
    }
}

encoding_struct! {
    /// Information about payment.
    struct Timestamp {
        const SIZE = 40;

        /// Hash of content.
        field content_hash:             &Hash       [00 => 32]
        /// Additional metadata.
        field metadata:                 &str        [32 => 40]
    }
}

encoding_struct! {
    /// User information entry.
    struct UserInfoEntry {
        const SIZE = 80;

        /// User information entry.
        field info:                     UserInfo    [00 => 08]
        /// Total amount of available transactions
        field available_timestamps:     i64         [08 => 16]
        /// Root hash of user timestamps.
        field timestamps_hash:          &Hash       [16 => 48]
        /// Root hash of user payments.
        field payments_hash:            &Hash       [48 => 80] 
    }
}

message! {
    /// Create or update user.
    struct TxUpdateUser {
        const TYPE = TIMESTAMPING_SERVICE_ID;
        const ID = TX_UPDATE_USER_ID;
        const SIZE = 8;

        /// User information content.
        field content:                  UserInfo    [00 => 08]
    }
}

message! {
    /// A payment transaction.
    struct TxPayment {
        const TYPE = TIMESTAMPING_SERVICE_ID;
        const ID = TX_PAYMENT_ID;
        const SIZE = 8;

        /// Information about payment.
        field content:                  PaymentInfo [00 => 08]
    }
}

message! {
    /// A timestamp transaction.
    struct TxTimestamp {
        const TYPE = TIMESTAMPING_SERVICE_ID;
        const ID = TX_TIMESTAMP;
        const SIZE = 8;

        /// Timestamp content.
        field content:                  Timestamp   [00 => 08]
    }
}