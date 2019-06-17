extern crate capnpc;


fn main() {
    ::capnpc::CompilerCommand::new()
    .edition(capnpc::RustEdition::Rust2018)
    .src_prefix("../schema")
    .file("../schema/cli.capnp")
    .run().expect("compiling schema");
}