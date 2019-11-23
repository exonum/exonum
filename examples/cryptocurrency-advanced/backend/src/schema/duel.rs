//! Duel.

use super::*;

use super::super::proto;

/// Поединок.
#[derive(Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Duel", serde_pb_convert)]
pub struct Duel {
    /// Ключ поединка.
    pub key: PublicKey,

    /// Ключ арбитра.
    pub arbiter_key: PublicKey,

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

    /// Количество транзакция связанных с поединком.
    pub history_len: u64,

    /// `Hash` of the transactions history.
    pub history_hash: Hash,
}

impl Duel {
    /// Создает поединок.
    pub fn new(
        &key: &PublicKey,
        &arbiter_key: &PublicKey,
        &player1_key: &PublicKey,
        &player2_key: &PublicKey,
        &judge1_key: &PublicKey,
        &judge2_key: &PublicKey,
        &judge3_key: &PublicKey,
        situation_number: u64,
        history_len: u64,
        &history_hash: &Hash,
    ) -> Self
    {
        Self {
            key,
            arbiter_key,
            player1_key,
            player2_key,
            judge1_key,
            judge2_key,
            judge3_key,
            situation_number,
            history_len,
            history_hash,
        }
    }
}

impl<T> Schema<T>
where
    T: IndexAccess,
{
    /// Возвращает поединки.
    pub fn duels(&self) -> ProofMapIndex<T, PublicKey, Duel> {
        ProofMapIndex::new("mwf.duels", self.access.clone())
    }

    /// Возвращает поединок по ключу.
    pub fn duel(&self, key: &PublicKey) -> Option<Duel> {
        self.duels().get(key)
    }

    /// Возвращает историю по поединку.
    pub fn duel_history(&self, public_key: &PublicKey) -> ProofListIndex<T, Hash> {
        ProofListIndex::new_in_family(
            "mwf.duel_history",
            public_key,
            self.access.clone(),
        )
    }

    /// Создает поединок.
    pub fn create_duel(
        &mut self,
        key: &PublicKey,
        arbiter_key: &PublicKey,
        player1_key: &PublicKey,
        player2_key: &PublicKey,
        judge1_key: &PublicKey,
        judge2_key: &PublicKey,
        judge3_key: &PublicKey,
        situation_number: u64,
        transaction: &Hash
    )
    {
        let duel = {
            let mut history = self.duel_history(key);
            history.push(*transaction);
            let history_hash = history.object_hash();
            Duel::new(
                key,
                arbiter_key,
                player1_key,
                player2_key,
                judge1_key,
                judge2_key,
                judge3_key,
                situation_number,
                history.len(),
                &history_hash)
        };
        self.duels().put(key, duel);
    }
}
