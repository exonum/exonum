#[cfg(test)]
mod test {
    use snow::NoiseBuilder;
    use snow::types::Dh;
    use snow::wrappers::crypto_wrapper::Dh25519;

    use crypto::PUBLIC_KEY_LENGTH;
    use crypto::{convert_keypair_for_handshake, gen_keypair};

    #[test]
    fn test_convert_ed_to_curve_dh() {
        // Generate Ed25519 keys for initiator and responder.
        let (public_key_i, secret_key_i) = gen_keypair();
        let (public_key_r, secret_key_r) = gen_keypair();

        // Convert to Curve25519 keys.
        let (public_key_i, secret_key_i) =
            convert_keypair_for_handshake(public_key_i, secret_key_i).unwrap();
        let (public_key_r, secret_key_r) =
            convert_keypair_for_handshake(public_key_r, secret_key_r).unwrap();

        // Do DH
        let mut keypair_i: Dh25519 = Default::default();
        keypair_i.set(&secret_key_i[..PUBLIC_KEY_LENGTH]);
        let mut output_i = [0u8; PUBLIC_KEY_LENGTH];
        keypair_i.dh(public_key_r.as_ref(), &mut output_i);

        let mut keypair_r: Dh25519 = Default::default();
        keypair_r.set(&secret_key_r[..PUBLIC_KEY_LENGTH]);
        let mut output_r = [0u8; PUBLIC_KEY_LENGTH];
        keypair_r.dh(public_key_i.as_ref(), &mut output_r);

        assert_eq!(output_i, output_r);
    }

    #[test]
    fn test_converted_keys_handshake() {
        const MSG_SIZE: usize = 4096;
        static PATTERN: &'static str = "Noise_XK_25519_ChaChaPoly_SHA256";

        // Handshake initiator keypair.
        let (public_key_i, secret_key_i) = gen_keypair();
        // Handshake responder keypair.
        let (public_key_r, secret_key_r) = gen_keypair();

        // Convert to Curve25519 keys.
        let (_, secret_key_i) = convert_keypair_for_handshake(public_key_i, secret_key_i).unwrap();
        let (public_key_r, secret_key_r) =
            convert_keypair_for_handshake(public_key_r, secret_key_r).unwrap();

        let mut h_i = NoiseBuilder::new(PATTERN.parse().unwrap())
            .local_private_key(&secret_key_i[..PUBLIC_KEY_LENGTH])
            .remote_public_key(&public_key_r[..])
            .build_initiator()
            .unwrap();

        let mut h_r = NoiseBuilder::new(PATTERN.parse().unwrap())
            .local_private_key(&secret_key_r[..PUBLIC_KEY_LENGTH])
            .build_responder()
            .unwrap();

        let mut buffer_msg = [0u8; MSG_SIZE * 2];
        let mut buffer_out = [0u8; MSG_SIZE * 2];

        let len = h_i.write_message(&[0u8; 0], &mut buffer_msg).unwrap();
        h_r.read_message(&buffer_msg[..len], &mut buffer_out)
            .unwrap();
        let len = h_r.write_message(&[0u8; 0], &mut buffer_msg).unwrap();
        h_i.read_message(&buffer_msg[..len], &mut buffer_out)
            .unwrap();
        let len = h_i.write_message(&[0u8; 0], &mut buffer_msg).unwrap();
        h_r.read_message(&buffer_msg[..len], &mut buffer_out)
            .unwrap();

        h_i.into_transport_mode().unwrap();
    }
}
