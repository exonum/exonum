#![allow(dead_code)]
use stream_struct::Field;

storage_value! {
    StructWithTwoSegments {
        const SIZE = 16;
        first:  &[u8]     [0 => 8]
        second: &[u8]     [8 => 16]
    }
}

#[test]
fn test_correct_storage_value() {
    let dat: Vec<u8> = vec![8u8, 0, 0, 0, 22, 0, 0, 0,
        18, 0, 0, 0,
        16, 0, 0, 0, 1, 0, 0, 0,
        17, 0, 0, 0, 1, 0, 0, 0,
        1, 2];
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0,
        17, 0, 0, 0, 1, 0, 0, 0,
        1, 2];
    let mut buffer = vec![0;8];
    test.write(&mut buffer, 0, 8);
    assert_eq!(buffer, dat);
    <StructWithTwoSegments as Field>::check(&dat, 0, 16).unwrap();
    let strukt = <StructWithTwoSegments as Field>::read(&dat, 0, 16);
    assert_eq!(strukt.first(), &[1u8]);
    assert_eq!(strukt.second(), &[2u8]);
}

#[test]
#[should_panic="OverlappingSegment"]
fn test_overlap_segments() {
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0,
        16, 0, 0, 0, 1, 0, 0, 0,
        1, 2];
    let mut buffer = vec![0;8];
    test.write(&mut buffer, 0, 8);
    <StructWithTwoSegments as Field>::check(&buffer, 0, 16).unwrap();
}

#[test]
#[should_panic="IncorrectSegmentReference"]
fn test_segments_reffer_header() {
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0,
        1, 0, 0, 0, 1, 0, 0, 0,
        1, 2];
    let mut buffer = vec![0;8];
    test.write(&mut buffer, 0, 8);
    <StructWithTwoSegments as Field>::check(&buffer, 0, 16).unwrap();
}

#[test]
#[should_panic="SpaceBetweenSegments"]
fn test_segments_has_spaces_between() {
    let test = vec![16u8, 0, 0, 0, 1, 0, 0, 0,
        18, 0, 0, 0, 1, 0, 0, 0, // <-- link after space
        1,
        0, // <-- this is space one
        2];
    let mut buffer = vec![0;8];
    test.write(&mut buffer, 0, 8);
    <StructWithTwoSegments as Field>::check(&buffer, 0, 16).unwrap();
}