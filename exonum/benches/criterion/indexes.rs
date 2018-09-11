// Copyright 2018 The Exonum Team
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

static KEY_SET_FAMILY_NAME: &str = "key";
static VALUE_SET_FAMILY_NAME: &str = "key";

use criterion::{
    AxisScale, Bencher, Criterion, ParameterizedBenchmark, PlotConfiguration, Throughput,
};
use exonum::{crypto, storage::{KeySetIndex, ValueSetIndex, Database, DbOptions, RocksDB}};
use tempdir::TempDir;
use num::pow::pow;

use std::{fs, path::{Path, PathBuf}};

pub fn key_value_set_index(c: &mut Criterion) {
    crypto::init();

    let tempdir = TempDir::new("exonum").unwrap();

    let path = tempdir.path().to_path_buf();
    c.bench(
        "insert",
        ParameterizedBenchmark::new("key_set_index", move |bencher, key_size| {
            key_set_insert(path.clone(), bencher, key_size)
        }, (0..16).map(|i| pow(2, i)))
            .throughput(|s| Throughput::Bytes(*s as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
    fs::remove_dir_all(tempdir.path()).unwrap();
    let path = tempdir.path().to_path_buf();
    c.bench(
        "insert",
        ParameterizedBenchmark::new("value_set_index", move |bencher, key_size| {
            value_set_insert(path.clone(), bencher, key_size)
        }, (6..16).map(|i| pow(2, i)))
            .throughput(|s| Throughput::Bytes(*s as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
    fs::remove_dir_all(tempdir.path()).unwrap();

    let path = tempdir.path().to_path_buf();
    c.bench(
        "contains",
        ParameterizedBenchmark::new("key_set_index", move |bencher, key_size| {
            key_set_contains(path.clone(), bencher, key_size)
        }, (6..16).map(|i| pow(2, i)))
            .throughput(|s| Throughput::Bytes(*s as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
    fs::remove_dir_all(tempdir.path()).unwrap();
    let path = tempdir.path().to_path_buf();
    c.bench(
        "contains",
        ParameterizedBenchmark::new("value_set_index", move |bencher, key_size| {
            value_set_contains(path.clone(), bencher, key_size)
        }, (6..16).map(|i| pow(2, i)))
            .throughput(|s| Throughput::Bytes(*s as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
    fs::remove_dir_all(tempdir.path()).unwrap();
    let path = tempdir.path().to_path_buf();
    c.bench(
        "contains",
        ParameterizedBenchmark::new("value_set_index: contains_by_hash", move |bencher, key_size| {
            value_set_contains_by_hash(path.clone(), bencher, key_size)
        }, (6..16).map(|i| pow(2, i)))
            .throughput(|s| Throughput::Bytes(*s as u32))
            .plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic)),
    );
    fs::remove_dir_all(tempdir.path()).unwrap();
}

fn create_rocksdb(path: &Path) -> RocksDB {
    let options = DbOptions::default();
    RocksDB::open(path, &options).unwrap()
}

fn key_set_insert(path: PathBuf, bencher: &mut Bencher, &key_size: &usize) {
    bencher.iter_with_setup(
        || (create_rocksdb(&path), (0..key_size).map(|x| (x % 255) as u8).collect::<Vec<u8>>()),
        |(db, data)| {
            let mut fork = db.fork();
            {
                let mut key_set = KeySetIndex::new(KEY_SET_FAMILY_NAME, &mut fork);
                key_set.insert(data);
            }
            db.merge_sync(fork.into_patch()).unwrap();
        }
    );
}

fn value_set_insert(path: PathBuf, bencher: &mut Bencher, &key_size: &usize) {
    bencher.iter_with_setup(
        || (create_rocksdb(&path), (0..key_size).map(|x| (x % 255) as u8).collect::<Vec<u8>>()),
        |(db, data)| {
            let mut fork = db.fork();
            {
                let mut value_set = ValueSetIndex::new(VALUE_SET_FAMILY_NAME, &mut fork);
                value_set.insert(data);
            }
            db.merge_sync(fork.into_patch()).unwrap();
        }
    );
}

fn key_set_contains(path: PathBuf, bencher: &mut Bencher, &key_size: &usize) {
    bencher.iter_with_setup(
        || (create_rocksdb(&path), (0..key_size).map(|x| (x % 255) as u8).collect::<Vec<u8>>()),
        |(db, data)| {
            let mut fork = db.fork();
            {
                let mut key_set: KeySetIndex<_, Vec<u8>> = KeySetIndex::new(KEY_SET_FAMILY_NAME, &mut fork);
                key_set.contains(&data);
            }
            db.merge_sync(fork.into_patch()).unwrap();
        }
    );
}

fn value_set_contains(path: PathBuf, bencher: &mut Bencher, &key_size: &usize) {
    bencher.iter_with_setup(
        || (create_rocksdb(&path), (0..key_size).map(|x| (x % 255) as u8).collect::<Vec<u8>>()),
        |(db, data)| {
            let mut fork = db.fork();
            {
                let mut value_set = ValueSetIndex::new(VALUE_SET_FAMILY_NAME, &mut fork);
                value_set.contains(&data);
            }
            db.merge_sync(fork.into_patch()).unwrap();
        }
    );
}

fn value_set_contains_by_hash(path: PathBuf, bencher: &mut Bencher, &key_size: &usize) {
    bencher.iter_with_setup(
        || (create_rocksdb(&path), crypto::hash(&(0..key_size).map(|x| (x % 255) as u8).collect::<Vec<u8>>())),
        |(db, hash)| {
            let mut fork = db.fork();
            {
                let mut value_set: ValueSetIndex<_, Vec<u8>> = ValueSetIndex::new(VALUE_SET_FAMILY_NAME, &mut fork);
                value_set.contains_by_hash(&hash);
            }
            db.merge_sync(fork.into_patch()).unwrap();
        }
    );
}
