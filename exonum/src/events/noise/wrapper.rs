use snow::NoiseBuilder;
use snow::Session;
use events::noise::HandshakeParams;
use std::fmt;
use std::fmt::{Error, Formatter};
use bytes::BytesMut;
use byteorder::{ByteOrder, LittleEndian};
use std::io;

pub const NOISE_MAX_MESSAGE_LEN: usize = 65_535;
pub const TAGLEN: usize = 16;
pub const HEADER_LEN: usize = 4;
pub const HANDSHAKE_HEADER_LEN: usize = 2;

// We choose XX pattern since it provides mutual authentication and
// transmission of static public keys.
// see: https://noiseprotocol.org/noise.html#interactive-patterns
static PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// Wrapper around noise session to provide latter convenient interface.
pub struct NoiseWrapper {
    pub session: Session,
}

impl NoiseWrapper {
    pub fn responder(params: &HandshakeParams) -> Self {
        let builder: NoiseBuilder = Self::noise_builder(params);
        let private_key = builder.generate_private_key().unwrap();
        let session = builder
            .local_private_key(&private_key)
            .build_responder()
            .unwrap();

        NoiseWrapper { session }
    }

    pub fn initiator(params: &HandshakeParams) -> Self {
        let builder: NoiseBuilder = Self::noise_builder(params);
        let private_key = builder.generate_private_key().unwrap();
        let session = builder
            .local_private_key(&private_key)
            .build_initiator()
            .unwrap();

        NoiseWrapper { session }
    }

    pub fn read_handshake_msg(&mut self, input: &[u8]) -> Result<(usize, Vec<u8>), NoiseError> {
        self.read(input, NOISE_MAX_MESSAGE_LEN)
    }

    pub fn write_handshake_msg(&mut self) -> Result<(usize, Vec<u8>), NoiseError> {
        // Payload in handshake messages can be empty.
        self.write(&[0u8])
    }

    pub fn into_transport_mode(self) -> Result<Self, NoiseError> {
        // Transition into transport mode after handshake is finished.
        let session = self.session.into_transport_mode().map_err(|e| {
            NoiseError::new(format!(
                "Error when converting session into transport mode {}.",
                e
            ))
        })?;
        Ok(NoiseWrapper { session })
    }

    pub fn decrypt_msg(&mut self, len: usize, buf: &mut BytesMut) -> Result<BytesMut, io::Error> {
        let data = buf.split_to(len + HEADER_LEN).to_vec();
        let data = &data[HEADER_LEN..];
        let mut decoded_message = vec![0u8; 0];

        data.chunks(NOISE_MAX_MESSAGE_LEN).for_each(|msg| {
            let len_to_read = if msg.len() == NOISE_MAX_MESSAGE_LEN {
                msg.len() - TAGLEN
            } else {
                msg.len()
            };

            let (_, read_to) = self.read(msg, len_to_read).unwrap();
            decoded_message.extend_from_slice(&read_to);
        });

        Ok(BytesMut::from(decoded_message))
    }

    pub fn encrypt_msg(&mut self, msg: &[u8], buf: &mut BytesMut) -> Result<Option<()>, io::Error> {
        let mut len = 0usize;
        let mut encoded_message = vec![0u8; 0];

        msg.chunks(NOISE_MAX_MESSAGE_LEN - TAGLEN).for_each(|msg| {
            let (written_bytes, written) = self.write(msg).unwrap();
            encoded_message.extend_from_slice(&written);
            len += written_bytes;
        });

        let mut msg_len_buf = vec![0u8; HEADER_LEN];

        LittleEndian::write_u32(&mut msg_len_buf, len as u32);
        let encoded_message = &encoded_message[0..len];
        msg_len_buf.extend_from_slice(encoded_message);
        buf.extend_from_slice(&msg_len_buf);
        Ok(None)
    }

    fn read(&mut self, input: &[u8], len: usize) -> Result<(usize, Vec<u8>), NoiseError> {
        let mut buf = vec![0u8; len];
        let len = self.session
            .read_message(input, &mut buf)
            .map_err(|e| NoiseError::new(format!("Error while reading noise message: {:?}", e.0)))?;
        Ok((len, buf))
    }

    fn write(&mut self, msg: &[u8]) -> Result<(usize, Vec<u8>), NoiseError> {
        let mut buf = vec![0u8; NOISE_MAX_MESSAGE_LEN];
        let len = self.session
            .write_message(msg, &mut buf)
            .map_err(|e| NoiseError::new(format!("Error while writing noise message: {:?}", e.0)))?;
        Ok((len, buf))
    }

    fn noise_builder(params: &HandshakeParams) -> NoiseBuilder {
        let public_key = params.public_key.as_ref();
        NoiseBuilder::new(PARAMS.parse().unwrap()).remote_public_key(public_key)
    }
}

impl fmt::Debug for NoiseWrapper {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        write!(
            f,
            "NoiseWrapper {{ handshake finished: {} }}",
            self.session.is_handshake_finished()
        )
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
