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

//! An implementation of routines for exporting a whole DB storage into
//! compressed CSV image and importing image into the DB.

/// Single record from DB image.
#[derive(Debug)]
pub struct DbImageRecord {
    table_name: String,
    key: Vec<u8>,
    value: Vec<u8>,
}

/// Helper routines to import and export of a DB image.
pub mod helpers {
    use storage::{Database, Result};
    use flate2::{Compression, write::GzEncoder, read::GzDecoder};
    use csv;
    use std::{self, sync::Arc};

    /// Determines number of records to be inserted in patch during image export.
    /// This affects RAM usage during image import and may be tuned.
    const RECORDS_PER_PATCH: usize = 1024;

    /// Reads records from the image and puts them into Database.
    pub fn import_db_from_image<R, D>(data: R, db: &D) -> Result<()>
        where R: std::io::Read, D: Database + 'static {

        // Construct records iterator from compressed CSV
        let decoder = GzDecoder::new(data);

        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(decoder);

        let mut records_read = 0usize;
        let mut fork = db.fork();

        for record in rdr.byte_records() {
            let record = record.unwrap();

            let record = super::DbImageRecord {
                table_name: String::from_utf8(record[0].to_vec()).unwrap(),
                key: record[1].to_vec(),
                value: record[2].to_vec(),
            };

            fork.put(&record.table_name, record.key, record.value);

            records_read += 1;

            // Insert records in batches of RECORDS_PER_PATCH records
            if records_read >= RECORDS_PER_PATCH {
                records_read = 0;

                db.merge(fork.into_patch())?;

                // Fork for the next batch of records
                fork = db.fork();
            }
        }

        // Some records might left after last merge, merge them as well
        if records_read > 0 {
            db.merge(fork.into_patch())?;
        }

        Ok(())
    }

    /// Reads records from the DB and puts them into compressed image.
    pub fn export_db_to_image<W: std::io::Write>(data: &mut W, db: Arc<dyn Database>) -> Result<()> {
        let snapshot = db.snapshot();
        let tables = snapshot.tables();

        let encoder = GzEncoder::new(data, Compression::best());
        let mut writer  = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(encoder);

        for table in tables {
            let mut iter = snapshot.iter(&table, &[]);

            loop {
                if let Some((k, v)) = iter.peek() {
                    let mut record = csv::ByteRecord::new();

                    // Insert CSV row that is basically a triple of: column family, key and value
                    record.push_field(&table.as_bytes());
                    record.push_field(&k);
                    record.push_field(&v);

                    writer.write_byte_record(&record).unwrap();
                } else {
                    break
                }

                iter.next();
            }
        }

        writer.flush().unwrap();
        writer.into_inner().unwrap().finish().unwrap();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use storage::Database;
    use super::helpers;
    use std::sync::Arc;

    mod memorydb_tests {
        use super::super::super::MemoryDB;

        fn memorydb_database() -> MemoryDB {
            MemoryDB::new()
        }

        #[test]
        fn memorydb_test_export_import_image() {
            let db = memorydb_database();
            super::test_export_import_image(db);
        }
    }

    mod rocksdb_tests {
        use super::super::super::{DbOptions, RocksDB};
        use std::path::Path;
        use tempdir::TempDir;

        fn rocksdb_database(path: &Path) -> RocksDB {
            let options = DbOptions::default();
            RocksDB::open(path, &options).unwrap()
        }

        #[test]
        fn rocksdb_export_import_image() {
            let dir = TempDir::new("exonum_rocksdb1").unwrap();
            let path = dir.path();

            let db = rocksdb_database(&path);
            super::test_export_import_image(db);
        }
    }

    /// Creates records in DB, clears DB, exports records into the image
    /// and imports that image back into DB.
    pub fn test_export_import_image<D: Database>(db: D) {
        let db = Arc::new(db);
        let mut fork = db.fork();

        let mut num_records = 0;

        // Create some records
        for i in 0..50 {
            for j in 0..50 {
                for k in 0..50 {
                    fork.put("t1", vec![i, j, k], vec![i, j, k]);
                    fork.put("t2", vec![i, j, k], vec![i, j, k]);
                    fork.put("t3", vec![i, j, k], vec![i, j, k]);

                    num_records += 3;
                }
            }
        }

        // Put records into the DB
        db.merge(fork.into_patch()).unwrap();

        println!("Exporting {} records...", num_records);

        let mut image_bytes: Vec<u8> = Vec::new();
        helpers::export_db_to_image(&mut image_bytes, db.clone()).unwrap();

        let record_size = 2 + 3 + 3 + 4; // 2 for table name, 3 for key, 3 for value, 4 for delimiters
        let raw_bytes_size = num_records * record_size;
        let saving = 100.0f32 - (image_bytes.len() as f32 / raw_bytes_size as f32 * 100.0f32);
        println!("Image size: {} bytes vs {} bytes uncompressed ({:.*}% compression)",
                 image_bytes.len(), raw_bytes_size, 1, saving);

        db.clear().unwrap();

        // Check that DB is empty after clearing
        {
            let snapshot = db.snapshot();
            assert!(!snapshot.contains("t1", &[0]));
            assert!(!snapshot.contains("t2", &[0]));
            assert!(!snapshot.contains("t3", &[0]));
        }

        helpers::import_db_from_image(&image_bytes[..], &*db).unwrap();

        // Check that records were imported
        {
            let snapshot = db.snapshot();

            for i in 0..50 {
                for j in 0..50 {
                    for k in 0..50 {
                        assert_eq!(snapshot.get("t1", &[i, j, k]), Some(vec![i, j, k]));
                        assert_eq!(snapshot.get("t2", &[i, j, k]), Some(vec![i, j, k]));
                        assert_eq!(snapshot.get("t3", &[i, j, k]), Some(vec![i, j, k]));
                    }
                }
            }
        }
    }
}
