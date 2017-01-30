use rand::{XorShiftRng, Rng, SeedableRng};

use exonum::crypto::{PublicKey, SecretKey, gen_keypair};

use ::timestamping::TimestampTx;

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