use rand::{XorShiftRng, Rng, SeedableRng};

use super::crypto::{PublicKey, SecretKey, Seed, gen_keypair_from_seed};
use super::messages::{TxMessage, TxIssue, TxTransfer};

pub struct TxGenerator {
    rand: XorShiftRng,
    clients: Vec<((PublicKey, SecretKey), &'static str)>
}

impl TxGenerator {
    pub fn new() -> TxGenerator {
        let mut rand = XorShiftRng::from_seed([192, 168, 56, 1]);

        fn seed(rand: &mut XorShiftRng) -> Seed {
            Seed::from_slice(&rand.gen::<[u8; 32]>()).unwrap()
        }

        let clients = vec![
            (gen_keypair_from_seed(&seed(&mut rand)), "USD"),
            (gen_keypair_from_seed(&seed(&mut rand)), "EUR"),
            (gen_keypair_from_seed(&seed(&mut rand)), "UAH"),
            (gen_keypair_from_seed(&seed(&mut rand)), "RUB"),
        ];

        TxGenerator {
            rand: rand,
            clients: clients
        }
    }
}

impl Iterator for TxGenerator {
    type Item = TxMessage;

    fn next(&mut self) -> Option<TxMessage> {
        let &((ref public_key, ref secret_key), ref name) =
            self.rand.choose(&self.clients).unwrap();
        if self.rand.gen_weighted_bool(10) {
            let seed = self.rand.gen();
            let amount = self.rand.gen_range(0, 100_000);
            Some(TxMessage::Issue(TxIssue::new(
                seed, public_key, name, amount, secret_key
            )))
        } else {
            let seed = self.rand.gen();
            let ref reciever = (self.rand.choose(&self.clients).unwrap().0).0;
            let amount = self.rand.gen_range(0, 1_000);
            Some(TxMessage::Transfer(TxTransfer::new(
                seed, public_key, reciever, amount, secret_key
            )))
        }
    }
}
