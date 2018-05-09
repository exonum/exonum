use snow::NoiseBuilder;
use snow::params::NoiseParams;
use snow::Session;
use crypto::PublicKey;

pub const NOISE_MAX_MESSAGE_LEN: usize = 65535;
pub const TAGLEN: usize = 16;
pub const HEADER_LEN: usize = 4;
pub const HANDSHAKE_HEADER_LEN: usize = 2;

lazy_static! {
    static ref PARAMS: NoiseParams = "Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap();
}

#[derive(Debug, Copy, Clone)]
pub struct NoiseKeyWrapper {
    pub public_key: PublicKey,
}

#[allow(missing_debug_implementations)]
pub struct NoiseWrapper {
    pub session: Session,
}

impl NoiseWrapper {
    pub fn responder(keys: &NoiseKeyWrapper) -> Self {
        let builder: NoiseBuilder =
            NoiseBuilder::new(PARAMS.clone()).remote_public_key(keys.public_key.as_ref());
        let private_key = builder.generate_private_key().unwrap();
        let session = builder
            .local_private_key(&private_key)
            .build_responder()
            .unwrap();

        NoiseWrapper {
            session
        }
    }

    pub fn initiator(keys: &NoiseKeyWrapper) -> Self {
        let builder: NoiseBuilder =
            NoiseBuilder::new(PARAMS.clone()).remote_public_key(keys.public_key.as_ref());
        let private_key = builder.generate_private_key().unwrap();
        let session = builder
            .local_private_key(&private_key)
            .build_initiator()
            .unwrap();

        NoiseWrapper {
            session,
        }
    }

    pub fn read(&mut self, input: Vec<u8>, len: usize) ->  Result<(usize, Vec<u8>), NoiseError> {
        let mut buf = vec![0u8; len];
        let len = self.session.read_message(&input, &mut buf).map_err(|_e| {
            NoiseError::new("Error while reading noise message.")
        })?;
        Ok((len, buf))
    }

    pub fn write(&mut self, msg: Vec<u8>) -> Result<(usize, Vec<u8>), NoiseError>  {
        let mut buf = vec![0u8; NOISE_MAX_MESSAGE_LEN];
        let len = self.session.write_message(&msg, &mut buf).map_err(|_e| {
            NoiseError::new("Error while writing noise message.")
        })?;
        Ok((len, buf))
    }

    pub fn red_handshake_msg(&mut self, input: Vec<u8>) -> Result<(usize, Vec<u8>), NoiseError> {
        self.read(input, NOISE_MAX_MESSAGE_LEN)
    }

    pub fn write_handshake_msg(&mut self) -> Result<(usize, Vec<u8>), NoiseError> {
        self.write(vec![0u8])
    }

    pub fn into_transport_mode(self) -> Result<Self, NoiseError> {
        let session = self.session.into_transport_mode().map_err(|_| {
           NoiseError::new("Error when converting session into transport mode.")
        })?;
        Ok(NoiseWrapper { session })
    }

}
#[derive(Fail, Debug, Clone)]
#[fail(display = "{}", message)]
pub struct NoiseError {
    message: String,
}

impl NoiseError {
    pub fn new<T: Into<String>>(message: T) -> Self {
        NoiseError {
            message: message.into(),
        }
    }
}
