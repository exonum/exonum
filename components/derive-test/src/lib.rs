use exonum_proto_derive::{protobuf_convert};
use exonum_proto::ProtobufConvert;

mod proto;

#[protobuf_convert(source = "proto::Point")]
struct Point {
    x: u32,
    y: u32,
}

#[test]
fn point_pb() {
    let point = Point { x: 1, y: 2 };

    point.to_pb();
}

