use rand::{XorShiftRng, Rng, SeedableRng};

use exonum::crypto::{PublicKey, SecretKey, gen_keypair};

use timestamping::TimestampTx;

pub struct TimestampingTxGenerator {
    rand: XorShiftRng,
    data_size: usize,
    public_key: PublicKey,
    secret_key: SecretKey,
}

impl TimestampingTxGenerator {
    pub fn new(data_size: usize) -> TimestampingTxGenerator {
        let rand = XorShiftRng::from_seed([192, 168, 56, 1]);
        let (public_key, secret_key) = gen_keypair();

        TimestampingTxGenerator {
            rand: rand,
            data_size: data_size,
            public_key: public_key,
            secret_key: secret_key,
        }
    }
}

impl Iterator for TimestampingTxGenerator {
    type Item = TimestampTx;

    fn next(&mut self) -> Option<TimestampTx> {
        let mut data = vec![0; self.data_size];
        self.rand.fill_bytes(&mut data);
        Some(TimestampTx::new(&self.public_key, &data, &self.secret_key))
    }
}

// pub struct CurrencyTxGenerator {
//     rand: XorShiftRng,
//     clients: Vec<((PublicKey, SecretKey), &'static str)>
// }

// impl CurrencyTxGenerator {
//     pub fn new() -> CurrencyTxGenerator {
//         let mut rand = XorShiftRng::from_seed([192, 168, 56, 1]);

//         let clients = vec![
//             (gen_keypair(), "USD"),
//             (gen_keypair(), "EUR"),
//             (gen_keypair(), "UAH"),
//             (gen_keypair(), "RUB"),
//         ];

//         CurrencyTxGenerator {
//             rand: rand,
//             clients: clients
//         }
//     }
// }

// impl Iterator for CurrencyTxGenerator {
//     type Item = TxMessage;

//     fn next(&mut self) -> Option<TxMessage> {
//         let &((ref public_key, ref secret_key), ref name) =
//             self.rand.choose(&self.clients).unwrap();
//         if self.rand.gen_weighted_bool(10) {
//             let seed = self.rand.gen();
//             let amount = self.rand.gen_range(0, 100_000);
//             Some(TxMessage::Issue(TxIssue::new(
//                 seed, public_key, name, amount, secret_key
//             )))
//         } else {
//             let seed = self.rand.gen();
//             let ref reciever = (self.rand.choose(&self.clients).unwrap().0).0;
//             let amount = self.rand.gen_range(0, 1_000);
//             Some(TxMessage::Transfer(TxTransfer::new(
//                 seed, public_key, reciever, amount, secret_key
//             )))
//         }
//     }
// }
