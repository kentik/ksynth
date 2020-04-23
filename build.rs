use capnpc::CompilerCommand;

fn main() {
    CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/chf.capnp")
        .run()
        .unwrap();
}
