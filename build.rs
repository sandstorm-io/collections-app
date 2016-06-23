extern crate capnpc;

fn main() {
    ::capnpc::compile("schema",
                      &["schema/collections.capnp"]).expect("compiling");
}

