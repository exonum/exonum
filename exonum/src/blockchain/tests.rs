#![allow(dead_code)]

#[test]
fn test_u64() {
    storage_value! {
        struct Test {
            const SIZE = 8;
            field some_test:u64 [0 => 8]
        }
    }
    let test_data = r##"{"some_test":"1234"}"##;
    let test = Test::new(1234);
    let data = ::serde_json::to_string(&test).unwrap();
    println!("{:?}", data);
    assert_eq!(data, test_data);
}

#[test]
fn test_system_time() {
    use std::time::{SystemTime, UNIX_EPOCH};
    storage_value! {
        struct Test {
            const SIZE = 12;
            field some_test:SystemTime [0 => 12]
        }
    }
    let test_data = r##"{"some_test":{"secs":"0","nanos":0}}"##;


    let test = Test::new(UNIX_EPOCH);
    let data = ::serde_json::to_string(&test).unwrap();
    assert_eq!(data, test_data);
}

use stream_struct::Field;

storage_value! {
    struct StructWithTwoSegments {
        const SIZE = 16;
        field first:  &[u8]     [0 => 8]
        field second: &[u8]     [8 => 16]
    }
}

#[test]
fn test_correct_storage_value() {
    let dat: Vec<u8> = vec![8u8, 0, 0, 0, 18, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 17, 0, 0, 0, 1, 0,
                            0, 0, 1, 2];
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0, 17, 0, 0, 0, 1, 0, 0, 0, 1, 2];
    let mut buffer = vec![0;8];
    test.write(&mut buffer, 0, 8);
    assert_eq!(buffer, dat);
    <StructWithTwoSegments as Field>::check(&dat, 0.into(), 16.into()).unwrap();
    let strukt = unsafe { <StructWithTwoSegments as Field>::read(&dat, 0, 16) };
    assert_eq!(strukt.first(), &[1u8]);
    assert_eq!(strukt.second(), &[2u8]);
}

#[test]
#[should_panic(expected ="OverlappingSegment")]
fn test_overlap_segments() {
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 1, 2];
    let mut buffer = vec![0;8];
    test.write(&mut buffer, 0, 8);
    <StructWithTwoSegments as Field>::check(&buffer, 0.into(), 16.into()).unwrap();
}


#[test]
#[should_panic(expected ="SpaceBetweenSegments")]
fn test_segments_has_spaces_between() {
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0,
        18, 0, 0, 0, 1, 0, 0, 0, // <-- link after space
        1,
        0, // <-- this is space one
        2];
    let mut buffer = vec![0;8];
    test.write(&mut buffer, 0, 8);
    <StructWithTwoSegments as Field>::check(&buffer, 0.into(), 16.into()).unwrap();
}
