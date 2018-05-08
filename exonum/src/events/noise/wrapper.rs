use snow::wrappers::crypto_wrapper::Dh25519;
use snow::wrappers::rand_wrapper::RandomOs;
use snow::types::Dh;
use snow::Session;
use snow::NoiseBuilder;
use snow::params::NoiseParams;

pub const NOISE_MAX_MESSAGE_LEN: usize = 65535;
pub const TAGLEN : usize = 16;
pub const HEADER_LEN : usize = 4;
pub const HANDSHAKE_HEADER_LEN: usize = 2;

lazy_static! {
    static ref PARAMS: NoiseParams = "Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap();
}

#[allow(missing_debug_implementations)]
pub struct NoiseWrapper {
    pub session: Session,
}

impl NoiseWrapper {
    pub fn responder() -> Self {
        let builder: NoiseBuilder = NoiseBuilder::new(PARAMS.clone());
        let mut static_i: Dh25519 = Default::default();
        let mut rand = RandomOs::default();
        static_i.generate(&mut rand);

        let mut session = builder
            .local_private_key(&static_i.privkey())
            .remote_public_key(&static_i.pubkey())
            .build_responder()
            .unwrap();

        NoiseWrapper {
            session
        }
    }

    pub fn initiator() -> Self {
        let builder: NoiseBuilder = NoiseBuilder::new(PARAMS.clone());
        let mut static_i: Dh25519 = Default::default();
        let mut rand = RandomOs::default();
        static_i.generate(&mut rand);

        let mut session = builder
            .local_private_key(&static_i.privkey())
            .remote_public_key(&static_i.pubkey())
            .build_initiator()
            .unwrap();

        NoiseWrapper {
            session
        }
    }

    pub fn read(&mut self, input: Vec<u8>, len:usize) -> (usize, Vec<u8>) {
        let mut buf = vec![0u8; len];
        let len = self.session.read_message(&input, &mut buf).unwrap();
        (len, buf)
    }

    pub fn write(&mut self, msg: Vec<u8>) -> Option<(usize, Vec<u8>)> {
        let mut buf = vec![0u8; NOISE_MAX_MESSAGE_LEN];
        let len = self.session.write_message(&msg, &mut buf).unwrap();
        Some((len, buf))
    }

    pub fn red_handshake_msg(&mut self, input: Vec<u8>) -> (usize, Vec<u8>) {
        self.read(input, NOISE_MAX_MESSAGE_LEN)
    }

    pub fn write_handshake_msg(&mut self) -> Option<(usize, Vec<u8>)> {
        self.write(vec![0u8])
    }

    pub fn into_transport_mode(self) -> Result<Self, ()> {
        Ok(NoiseWrapper { session: self.session.into_transport_mode().unwrap() })
    }
}
