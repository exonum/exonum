use router::Router;
use serde_json;

use blockchain::{Block, SCHEMA_MAJOR_VERSION};
use crypto::Hash;

use super::*;

#[test]
fn test_json_response_for_complex_val() {
    let str_val = "sghdkgskgskldghshgsd";
    let txs = [34, 32];
    let tx_count = txs.len() as u32;
    let complex_val = Block::new(
        SCHEMA_MAJOR_VERSION,
        0,
        24,
        tx_count,
        &Hash::new([24; 32]),
        &Hash::new([34; 32]),
        &Hash::new([38; 32]),
    );
    struct SampleAPI;
    impl Api for SampleAPI {
        fn wire<'b>(&self, _: &'b mut Router) {
            return;
        }
    }
    let stub = SampleAPI;
    let result = stub.ok_response(&serde_json::to_value(str_val).unwrap());
    assert!(result.is_ok());
    let result = stub.ok_response(&serde_json::to_value(&complex_val).unwrap());
    assert!(result.is_ok());
    print!("{:?}", result);
}
