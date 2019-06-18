extern crate capnpc;

fn main() {
    capnpc::CompilerCommand::new()
        .file("external.capnp")
        .run()
        .expect("schema compiler command");
}
