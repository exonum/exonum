//! CreateDuel.

// Workaround for `failure` see https://github.com/rust-lang-nursery/failure/issues/223 and
// ECR-1771 for the details.
#![allow(bare_trait_objects)]

use super::*;
//use failure::err_msg;

/// Транзакция создания поединка.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::CreateDuel")]
pub struct CreateDuel {
    /// Ключ поединка.
    pub key: PublicKey,

    /// Ключ играка 1.
    pub player1_key: PublicKey,
    /// Ключ играка 2.
    pub player2_key: PublicKey,

    /// Ключ судьи 1.
    pub judge1_key: PublicKey,
    /// Ключ судьи 2.
    pub judge2_key: PublicKey,
    /// Ключ судьи 3.
    pub judge3_key: PublicKey,

    /// Номер ситуации.
    pub situation_number: u64,
}

impl CreateDuel {
    #[doc(hidden)]
    pub fn sign(
        key: &PublicKey,
        player1_key: &PublicKey,
        player2_key: &PublicKey,
        judge1_key: &PublicKey,
        judge2_key: &PublicKey,
        judge3_key: &PublicKey,
        situation_number: u64,
        pk: &PublicKey,
        sk: &SecretKey
    ) -> Signed<RawTransaction>
    {
        Message::sign_transaction(
            Self {
                key: key.to_owned(),
                player1_key: player1_key.to_owned(),
                player2_key: player2_key.to_owned(),
                judge1_key: judge1_key.to_owned(),
                judge2_key: judge2_key.to_owned(),
                judge3_key: judge3_key.to_owned(),
                situation_number: situation_number.to_owned(),
            },
            CRYPTOCURRENCY_SERVICE_ID,
            *pk,
            sk,
        )
    }
}

impl Transaction for CreateDuel {
    fn execute(&self, context: TransactionContext) -> ExecutionResult {
        let arbiter_key = &context.author();
        let hash = context.tx_hash();

        let mut schema = Schema::new(context.fork());

        let key = self.key;

        if schema.duel(&key).is_some() {
            Err(Error::DuelAlreadyExists)?;
        }

        if self.player1_key == self.player2_key {
            Err(Error::NeedTwoPlayers)?;
        }

        if self.judge1_key == self.judge2_key || self.judge1_key == self.judge3_key || self.judge2_key == self.judge3_key {
            //Err(Error(err_msg("Поединок должны судить трое разных судей")))?;
        }

        schema.create_duel(
            &key,
            &arbiter_key,
            &self.player1_key,
            &self.player2_key,
            &self.judge1_key,
            &self.judge2_key,
            &self.judge3_key,
            self.situation_number,
            &hash
        );

        Ok(())
    }
}
