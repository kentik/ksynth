use anyhow::Result;
use capnpc::CompilerCommand;
use git2::{DescribeOptions, Repository};

fn main() -> Result<()> {
    let repo = Repository::open_from_env()?;
    let head = repo.head()?;

    if let Some(name) = head.name() {
        let path = repo.path().join(name);
        let path = path.to_string_lossy();
        println!("cargo:rerun-if-changed={}", path);
    }

    let mut opts = DescribeOptions::new();
    opts.describe_all();
    opts.describe_tags();
    opts.show_commit_oid_as_fallback(true);

    let commit  = head.peel_to_commit()?;
    let hash    = commit.id();
    let desc    = repo.describe(&opts)?;
    let version = desc.format(None)?;

    println!("cargo:rustc-env=GIT_COMMIT={}", hash);
    println!("cargo:rustc-env=GIT_VERSION={}", version);

    CompilerCommand::new()
        .src_prefix("schema")
        .file("schema/chf.capnp")
        .run()?;

    Ok(())
}
