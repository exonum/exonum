extern crate capnpc;

fn main() {
    ::capnpc::compile(".", &["protocol.capnp"]).unwrap();
}
