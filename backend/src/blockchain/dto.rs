use exonum::crypto::{PublicKey, Hash};

use TIMESTAMPING_SERVICE;

pub const TX_UPDATE_USER_ID: u16 = 0;
pub const TX_PAYMENT_ID: u16 = 1;
pub const TX_TIMESTAMP_ID: u16 = 2;

encoding_struct! {
    /// Information about timestapming user.
    struct UserInfo {
        /// Unique user identifier.
        id:                       &str,
        /// Public key of user.
        pub_key:                  &PublicKey,
        /// Encrypted secret key.
        encrypted_secret_key:     &[u8],
        /// Additional metadata.
        metadata:                 &str,
    }
}

encoding_struct! {
    /// Information about payment.
    struct PaymentInfo {
        /// User identifier.
        user_id:                  &str,
        /// Total amount of available transactions.
        total_amount:             u64,
        /// Additional metadata.
        metadata:                 &str,
    }
}

encoding_struct! {
    /// Information about payment.
    struct Timestamp {
        /// User identifier.
        user_id:                  &str,
        /// Hash of content.
        content_hash:             &Hash,
        /// Additional metadata.
        metadata:                 &str,
    }
}

encoding_struct! {
    /// User information entry.
    struct UserInfoEntry {
        /// User information entry.
        info:                     UserInfo,
        /// Total amount of available transactions
        available_timestamps:     i64,
        /// Root hash of user payments.
        payments_hash:            &Hash,
    }
}

encoding_struct! {
    /// Timestamp entry
    struct TimestampEntry {
        /// User identifier.
        timestamp:                Timestamp,
        /// Hash of tx.
        tx_hash:                  &Hash,
    }
}


message! {
    /// Create or update user.
    struct TxUpdateUser {
        const TYPE = TIMESTAMPING_SERVICE;
        const ID = TX_UPDATE_USER_ID;

        /// Public key of transaction.
        pub_key:                  &PublicKey,
        /// User information content.
        content:                  UserInfo,
    }
}

message! {
    /// A payment transaction.
    struct TxPayment {
        const TYPE = TIMESTAMPING_SERVICE;
        const ID = TX_PAYMENT_ID;

        /// Public key of transaction.
        pub_key:                  &PublicKey,
        /// Information about payment.
        content:                  PaymentInfo,
    }
}

message! {
    /// A timestamp transaction.
    struct TxTimestamp {
        const TYPE = TIMESTAMPING_SERVICE;
        const ID = TX_TIMESTAMP_ID;

        /// Public key of transaction.
        pub_key:                  &PublicKey,
        /// Timestamp content.
        content:                  Timestamp,
    }
}

// TODO content tests
