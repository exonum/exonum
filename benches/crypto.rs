#![feature(test)]

extern crate test;
extern crate da;

use test::Bencher;
use da::crypto::{gen_keypair, sign, verify};

#[bench]
fn bench_sign_64(b: &mut Bencher) {
    let (public_key, secret_key) = gen_keypair();
    let data = (0..64).collect::<Vec<u8>>();
    b.iter(|| {
        sign(&data, &secret_key)
    })
}

#[bench]
fn bench_sign_128(b: &mut Bencher) {
    let (public_key, secret_key) = gen_keypair();
    let data = (0..128).collect::<Vec<u8>>();
    b.iter(|| {
        sign(&data, &secret_key)
    })
}

#[bench]
fn bench_sign_1024(b: &mut Bencher) {
    let (public_key, secret_key) = gen_keypair();
    let data = (0..128).collect::<Vec<u8>>();
    b.iter(|| {
        sign(&data, &secret_key)
    })
}

#[bench]
fn bench_verify_64(b: &mut Bencher) {
    let (public_key, secret_key) = gen_keypair();
    let data = (0..64).collect::<Vec<u8>>();
    let signature = sign(&data, &secret_key);
    b.iter(|| {
        verify(&signature, &data, &public_key)
    })
}

#[bench]
fn bench_verify_128(b: &mut Bencher) {
    let (public_key, secret_key) = gen_keypair();
    let data = (0..128).collect::<Vec<u8>>();
    let signature = sign(&data, &secret_key);
    b.iter(|| {
        verify(&signature, &data, &public_key)
    })
}
#[bench]
fn bench_verify_1024(b: &mut Bencher) {
    let (public_key, secret_key) = gen_keypair();
    let data = (0..1024).collect::<Vec<u8>>();
    let signature = sign(&data, &secret_key);
    b.iter(|| {
        verify(&signature, &data, &public_key)
    })
}
