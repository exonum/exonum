#![feature(test)]

extern crate test;
extern crate exonum;

#[cfg(test)]
mod tests {
    use test::Bencher;
    use exonum::crypto::{gen_keypair, sign, verify, hash, HashStream, SignStream};

    #[bench]
    fn bench_sign_64(b: &mut Bencher) {
        let (_, secret_key) = gen_keypair();
        let data = (0..64).collect::<Vec<u8>>();
        b.iter(|| sign(&data, &secret_key))
    }

    #[bench]
    fn bench_sign_128(b: &mut Bencher) {
        let (_, secret_key) = gen_keypair();
        let data = (0..128).collect::<Vec<u8>>();
        b.iter(|| sign(&data, &secret_key))
    }

    #[bench]
    fn bench_sign_1024(b: &mut Bencher) {
        let (_, secret_key) = gen_keypair();
        let data = (0..1024)
            .map(|x| (x % 255) as u8)
            .collect::<Vec<_>>();
        b.iter(|| sign(&data, &secret_key))
    }

    #[bench]
    fn bench_sign_1024_inited_sodium(b: &mut Bencher) {
        ::exonum::crypto::init();
        let (_, secret_key) = gen_keypair();
        let data = (0..1024)
            .map(|x| (x % 255) as u8)
            .collect::<Vec<_>>();
        b.iter(|| sign(&data, &secret_key))
    }

    #[bench]
    fn bench_verify_64(b: &mut Bencher) {
        let (public_key, secret_key) = gen_keypair();
        let data = (0..64).collect::<Vec<u8>>();
        let signature = sign(&data, &secret_key);
        b.iter(|| verify(&signature, &data, &public_key))
    }

    #[bench]
    fn bench_verify_128(b: &mut Bencher) {
        let (public_key, secret_key) = gen_keypair();
        let data = (0..128).collect::<Vec<u8>>();
        let signature = sign(&data, &secret_key);
        b.iter(|| verify(&signature, &data, &public_key))
    }

    #[bench]
    fn bench_verify_1024(b: &mut Bencher) {
        let (public_key, secret_key) = gen_keypair();
        let data = (0..1024)
            .map(|x| (x % 255) as u8)
            .collect::<Vec<_>>();
        let signature = sign(&data, &secret_key);
        b.iter(|| verify(&signature, &data, &public_key))
    }

    #[bench]
    fn bench_verify_1024_inited_sodium(b: &mut Bencher) {
        ::exonum::crypto::init();
        let (public_key, secret_key) = gen_keypair();
        let data = (0..1024)
            .map(|x| (x % 255) as u8)
            .collect::<Vec<_>>();
        let signature = sign(&data, &secret_key);
        b.iter(|| verify(&signature, &data, &public_key))
    }

    #[bench]
    fn bench_hash_64(b: &mut Bencher) {
        let data = (0..64).collect::<Vec<u8>>();
        b.iter(|| hash(&data))
    }

    #[bench]
    fn bench_hash_128(b: &mut Bencher) {
        let data = (0..128).collect::<Vec<u8>>();
        b.iter(|| hash(&data))
    }

    #[bench]
    fn bench_hash_1024(b: &mut Bencher) {
        let data = (0..1024)
            .map(|x| (x % 255) as u8)
            .collect::<Vec<_>>();
        b.iter(|| hash(&data))
    }

    #[bench]
    fn bench_hash_1024_inited_sodium(b: &mut Bencher) {
        ::exonum::crypto::init();
        let data = (0..1024)
            .map(|x| (x % 255) as u8)
            .collect::<Vec<_>>();
        b.iter(|| hash(&data))
    }

    // const FILE: &'static str = "path_to_file_for_hashing";

    // #[bench]
    // fn bench_hash_file(b: &mut Bencher) {
    //     use std::io::{BufReader, Read, Seek, SeekFrom};
    //     use std::fs::File;
    //     use std::process::Command;

    //     let mut reader = BufReader::new(File::open(FILE).unwrap());
    //     let mut buffer = [0; 1024];

    //     b.iter(|| {
    //         let mut stream = HashStream::new();
    //         loop {
    //             let len = reader.read(&mut buffer).unwrap();
    //             if len == 0 {
    //                 break;
    //             }
    //             stream.update(&buffer[..len]);
    //         }
    //         let _ = stream.finalize();
    //         let _ = reader.seek(SeekFrom::Start(0));
    //     });
    // }

    // #[bench]
    // fn bench_sign_file(b: &mut Bencher) {
    //     use std::io::{BufReader, Read, Seek, SeekFrom};
    //     use std::fs::File;

    //     let mut reader = BufReader::new(File::open(FILE).unwrap());
    //     let mut buffer = [0; 1024];

    //     b.iter(|| {
    //         let mut create_stream = SignStream::new();
    //         let (_, sk) = gen_keypair();
    //         loop {
    //             let len = reader.read(&mut buffer).unwrap();
    //             if len == 0 {
    //                 break;
    //             }
    //             create_stream.update(&buffer[..len]);
    //         }
    //         let _ = create_stream.finalize(&sk);
    //         let _ = reader.seek(SeekFrom::Start(0));
    //     });
    // }
}
