// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![feature(test)]

extern crate test;
extern crate exonum;

#[cfg(test)]
mod tests {
    use test::Bencher;
    use exonum::crypto::{gen_keypair, sign, verify, hash};

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
        let data = (0..1024).map(|x| (x % 255) as u8).collect::<Vec<_>>();
        b.iter(|| sign(&data, &secret_key))
    }

    #[bench]
    fn bench_sign_1024_inited_sodium(b: &mut Bencher) {
        ::exonum::crypto::init();
        let (_, secret_key) = gen_keypair();
        let data = (0..1024).map(|x| (x % 255) as u8).collect::<Vec<_>>();
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
        let data = (0..1024).map(|x| (x % 255) as u8).collect::<Vec<_>>();
        let signature = sign(&data, &secret_key);
        b.iter(|| verify(&signature, &data, &public_key))
    }

    #[bench]
    fn bench_verify_1024_inited_sodium(b: &mut Bencher) {
        ::exonum::crypto::init();
        let (public_key, secret_key) = gen_keypair();
        let data = (0..1024).map(|x| (x % 255) as u8).collect::<Vec<_>>();
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
        let data = (0..1024).map(|x| (x % 255) as u8).collect::<Vec<_>>();
        b.iter(|| hash(&data))
    }

    #[bench]
    fn bench_hash_1024_inited_sodium(b: &mut Bencher) {
        ::exonum::crypto::init();
        let data = (0..1024).map(|x| (x % 255) as u8).collect::<Vec<_>>();
        b.iter(|| hash(&data))
    }
}
