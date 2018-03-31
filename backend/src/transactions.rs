use exonum::blockchain::{ExecutionResult, ExecutionError, Transaction};
use exonum::storage::Fork;
use exonum::crypto::{CryptoHash, PublicKey};
use exonum::messages::Message;

use CRYPTOCURRENCY_SERVICE_ID;
use schema::CurrencySchema;
use wallet::Wallet;

const INITIAL_BALANCE: u64 = 100;

/// Error codes emitted by wallet transactions during execution.
#[derive(Debug, Fail)]
#[repr(u8)]
pub enum Error {
    /// Wallet already exists.
    ///
    /// Can be emitted by `CreateWallet`.
    #[fail(display = "Wallet already exists")]
    WalletAlreadyExists = 0,

    /// Sender doesn't exist.
    ///
    /// Can be emitted by `Transfer`.
    #[fail(display = "Sender doesn't exist")]
    SenderNotFound = 1,

    /// Receiver doesn't exist.
    ///
    /// Can be emitted by `Transfer` or `Issue`.
    #[fail(display = "Receiver doesn't exist")]
    ReceiverNotFound = 2,

    /// Insufficient currency amount.
    ///
    /// Can be emitted by `Transfer`.
    #[fail(display = "Insufficient currency amount")]
    InsufficientCurrencyAmount = 3,
}

impl From<Error> for ExecutionError {
    fn from(value: Error) -> ExecutionError {
        let description = format!("{}", value);
        ExecutionError::with_description(value as u8, description)
    }
}

transactions! {
    pub WalletTransactions {
        const SERVICE_ID = CRYPTOCURRENCY_SERVICE_ID;

        /// Transfer `amount` of the currency from one wallet to another.
        struct Transfer {
            from:    &PublicKey,
            to:      &PublicKey,
            amount:  u64,
            seed:    u64,
        }

        /// Issue `amount` of the currency to the `wallet`.
        struct Issue {
            wallet:  &PublicKey,
            amount:  u64,
            seed:    u64,
        }

        /// Create wallet with the given `name`.
        struct CreateWallet {
            pub_key: &PublicKey,
            name:    &str,
        }
    }
}

impl Transaction for Transfer {
    fn verify(&self) -> bool {
        (*self.from() != *self.to()) && self.verify_signature(self.from())
    }

    fn execute(&self, fork: &mut Fork) -> ExecutionResult {
        let mut schema = CurrencySchema::new(fork);

        let from = self.from();
        let to = self.to();
        let hash = self.hash();
        let amount = self.amount();

        let sender = schema.wallet(from).ok_or_else(|| {
            Error::SenderNotFound
        })?;

        let receiver = schema.wallet(to).ok_or_else(|| {
            schema.append_failure(from, &hash);
            Error::ReceiverNotFound
        })?;

        if sender.balance() < amount {
            schema.append_failure(from, &hash);
            Err(Error::InsufficientCurrencyAmount)?
        }

        let sender = sender.decrease(amount);
        let receiver = receiver.increase(amount);

        schema.wallets_mut().put(from, sender);
        schema.wallets_mut().put(to, receiver);
        schema.append_success(from, &hash);

        Ok(())
    }
}

impl Transaction for Issue {
    fn verify(&self) -> bool {
        self.verify_signature(self.wallet())
    }

    fn execute(&self, fork: &mut Fork) -> ExecutionResult {
        let mut schema = CurrencySchema::new(fork);
        let pub_key = self.wallet();
        let hash = self.hash();

        if let Some(wallet) = schema.wallet(pub_key) {
            let amount = self.amount();
            let wallet = wallet.increase(amount);
            schema.wallets_mut().put(pub_key, wallet);
            schema.append_success(pub_key, &hash);
            Ok(())
        } else {
            Err(Error::ReceiverNotFound)?
        }
    }
}

impl Transaction for CreateWallet {
    fn verify(&self) -> bool {
        self.verify_signature(self.pub_key())
    }

    fn execute(&self, fork: &mut Fork) -> ExecutionResult {
        let mut schema = CurrencySchema::new(fork);
        let pub_key = self.pub_key();
        let hash = self.hash();

        if schema.wallet(pub_key).is_some() {
            schema.append_failure(pub_key, &hash);
            Err(Error::WalletAlreadyExists)?
        } else {
            let name = self.name();
            let wallet = Wallet::create(pub_key, name, INITIAL_BALANCE);
            schema.wallets_mut().put(pub_key, wallet);
            schema.append_success(pub_key, &hash);
            Ok(())
        }
    }
}
