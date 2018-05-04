use crypto::PublicKey;
use events::codec::MessagesCodec;
use events::noise::noise_codec::NoiseCodec;
use futures::future::Future;
use snow::NoiseBuilder;
use snow::params::NoiseParams;
use std::io;
use tokio;
use tokio_core::net::TcpStream;
use tokio_io::AsyncRead;
use tokio_io::codec::Framed;
use futures::future::ok;
use futures::Stream;

mod noise_codec;

static SECRET: &'static [u8] = b"secret secret secret key secrets";
lazy_static! {
    static ref PARAMS: NoiseParams = "Noise_XXpsk3_25519_ChaChaPoly_BLAKE2s".parse().unwrap();
}

#[derive(Debug)]
pub struct NoiseWrapper {
    pub max_message_len: u32,
}

#[cfg(all(feature = "noise_protocol"))]
impl NoiseWrapper {
    pub fn wrap(&self) {
        info!("wrap noise protocol")
    }

    pub fn listen_handshake(&self, stream: TcpStream, stored: &PublicKey) -> Box<Future<Item=Framed<TcpStream, NoiseCodec>, Error=io::Error>> {
        listen_handshake(stream, stored)
    }

    pub fn send_handshake(&self, stream: TcpStream, stored: &PublicKey) -> Box<Future<Item=Framed<TcpStream, NoiseCodec>, Error=io::Error>> {
        send_handshake(stream, stored)
    }

}

//TODO: Consider using tokio-proto for noise handshake
pub fn listen_handshake(stream: TcpStream,
                        stored: &PublicKey,
) -> Box<Future<Item=Framed<TcpStream, NoiseCodec>, Error=io::Error>> {
    let builder: NoiseBuilder = NoiseBuilder::new(PARAMS.clone());
    let static_key = stored.as_ref();
    let mut noise = builder
        .local_private_key(&static_key)
        .psk(3, SECRET)
        .build_responder()
        .unwrap();

    let framed = read(stream).and_then(move |(sock, msg)| {
        let mut buf = vec![0u8; 65535];
        // <- e
        let _res = noise.read_message(&msg, &mut buf);

        // -> e, ee, s, es
        let len = noise.write_message(&[0u8; 0], &mut buf).unwrap();

        write(sock, buf, len)
            .and_then(|(sock, _msg)| read(sock))
            .and_then(move |(sock, msg)| {
                let mut buf = vec![0u8; 65535];
                // <- s, se
                noise.read_message(&msg, &mut buf).unwrap();

                let noise = noise.into_transport_mode().unwrap();
                let framed = sock.framed(NoiseCodec::new(noise));
                Ok(framed)
            })
    });

    Box::new(framed)
}

pub fn send_handshake(stream: TcpStream,
                      stored: &PublicKey,
) -> Box<Future<Item=Framed<TcpStream, NoiseCodec>, Error=io::Error>> {
    let builder: NoiseBuilder = NoiseBuilder::new(PARAMS.clone());
    let static_key = stored.as_ref();
    let mut noise = builder
        .local_private_key(&static_key)
        .psk(3, SECRET)
        .build_initiator()
        .unwrap();

    let mut buf = vec![0u8; 65535];
    // -> e
    let len = noise.write_message(&[], &mut buf).unwrap();
    let framed = write(stream, buf, len)
        .and_then(|(sock, _msg)| read(sock))
        .and_then(|(sock, msg)| {
            let mut buf = vec![0u8; 65535];
            // <- e, ee, s, es
            noise.read_message(&msg, &mut buf).unwrap();

            let len = noise.write_message(&[], &mut buf).unwrap();
            let buf = &buf[0..len];
            write(sock, Vec::from(buf), len).and_then(|sock| {
                let noise = noise.into_transport_mode().unwrap();
                let framed = sock.0.framed(NoiseCodec::new(noise));
                Ok(framed)
            })
        });

    Box::new(framed)
}

pub fn read(sock: TcpStream) -> Box<Future<Item=(TcpStream, Vec<u8>), Error=io::Error>> {
    let buf = vec![0u8; 2];
    Box::new(
        tokio::io::read_exact(sock, buf)
            .and_then(|sock| tokio::io::read_exact(sock.0, vec![0u8; sock.1[1] as usize])),
    )
}

pub fn write(sock: TcpStream,
             buf: Vec<u8>,
             len: usize,
) -> Box<Future<Item=(TcpStream, Vec<u8>), Error=io::Error>> {
    let mut msg_len_buf = vec![(len >> 8) as u8, (len & 0xff) as u8];
    let buf = &buf[0..len];
    msg_len_buf.extend_from_slice(buf);
    Box::new(tokio::io::write_all(sock, msg_len_buf))
}

#[cfg(not(feature = "noise_protocol"))]
impl NoiseWrapper {
    pub fn wrap(&self) {
        info!("skip noise protocol")
    }

    pub fn listen_handshake(&self, stream: TcpStream, _: &PublicKey) -> Box<Future<Item=Framed<TcpStream, MessagesCodec>, Error=io::Error>> {
        self.framed_stream(stream)
    }

    pub fn send_handshake(&self, stream: TcpStream, _: &PublicKey) -> Box<Future<Item=Framed<TcpStream, MessagesCodec>, Error=io::Error>> {
        self.framed_stream(stream)
    }

    pub fn framed_stream(&self, stream: TcpStream) -> Box<Future<Item=Framed<TcpStream, MessagesCodec>, Error=io::Error>> {
        let framed = stream.framed(MessagesCodec::new(self.max_message_len));
        Box::new(ok(framed))
    }
}
