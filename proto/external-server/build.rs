extern crate capnpc;

fn main() {
    capnpc::CompilerCommand::new()
        .file("external.capnp")
        .edition(capnpc::RustEdition::Rust2018)
        .run()
        .expect("schema compiler command");
}
