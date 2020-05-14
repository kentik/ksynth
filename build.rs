use std::process::Command;
use anyhow::Result;
use capnpc::CompilerCommand;

fn main() -> Result<()> {
    let version = git(&["describe", "--always", "--tags", "--dirty"])?;
    let commit  = git(&["rev-parse", "HEAD"])?;

    println!("cargo:rustc-env=GIT_VERSION={}", version);
    println!("cargo:rustc-env=GIT_COMMIT={}", commit);

    CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/chf.capnp")
        .run()?;

    Ok(())
}

fn git(args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    for arg in args {
        cmd.arg(arg);
    }
    let output = cmd.output()?.stdout;
    Ok(String::from_utf8(output)?)
}
