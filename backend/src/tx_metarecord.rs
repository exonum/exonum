use exonum::crypto::Hash;

encoding_struct! {
/// Represents transaction. If `execution_status` equals to `true`, then the transaction
/// was successful.
    struct TxMetaRecord {
        tx_hash:                &Hash,
        execution_status:       bool,
    }
}

#[cfg(test)]
mod tests {
    use exonum::storage::StorageValue;
    use super::*;

    #[derive(Serialize)]
    struct TxMetaRecordTestData {
        json: TxMetaRecord,
        hash: Hash,
        raw: Vec<u8>,
    }

    impl TxMetaRecordTestData {
        fn new(data: TxMetaRecord) -> TxMetaRecordTestData {
            let hash = data.hash();
            let raw = StorageValue::into_bytes(data.clone());
            TxMetaRecordTestData {
                json: data,
                hash: hash,
                raw: raw,
            }
        }
    }

    #[test]
    fn test_tx_meta_record() {
        let hash = Hash::new([2; 32]);
        let status = false;
        let datum = TxMetaRecord::new(&hash, status);

        let datum = datum.clone();
        assert_eq!(datum.tx_hash(), &hash);
        assert_eq!(datum.execution_status(), status);
    }

    #[test]
    fn test_tx_meta_record_serde() {
        use serde_json;
        use rand::{thread_rng, Rng};
        use exonum::crypto::HASH_SIZE;

        let mut rng = thread_rng();
        let generator = move |_| {
            let mut hash_bytes = [0; HASH_SIZE];
            rng.fill_bytes(&mut hash_bytes);
            let hash = Hash::new(hash_bytes);
            let status = rng.gen_weighted_bool(2);
            TxMetaRecord::new(&hash, status)
        };
        let data = (0..50).map(generator).collect::<Vec<_>>();
        for datum in data {
            let json_str = serde_json::to_string(&datum).unwrap();
            let datum_round_trip: TxMetaRecord = serde_json::from_str(&json_str).unwrap();
            assert_eq!(datum, datum_round_trip);
            trace!(
                "TxMetaRecord test data: {}",
                serde_json::to_string(&TxMetaRecordTestData::new(datum)).unwrap()
            );
        }
    }
}
