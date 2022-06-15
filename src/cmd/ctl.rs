use anyhow::{anyhow, Result};
use crate::args::Args;
use crate::ctl::{Client, Command, Trace};
use crate::status::Report;

pub async fn ctl(args: Args<'_, '_>) -> Result<()> {
    let socket = args.arg::<String>("socket")?;
    let client = Client::new(socket).await?;

    match args.subcommand() {
        Some(("status", args)) => status(args, client).await,
        Some(("trace",  args)) => trace(args, client).await,
        Some(_) | None         => return Err(anyhow!("unsupported command")),
    }
}

async fn status(args: Args<'_, '_>, mut client: Client) -> Result<()> {
    let region = args.opt("region")?.unwrap_or_default();
    let report = client.send::<Report>(Command::Status(region)).await?;
    let output = match args.arg::<String>("output")?.as_str() {
        "json" => serde_json::to_string(&report)?,
        "yaml" => serde_yaml::to_string(&report)?,
        format => return Err(anyhow!("unsupported format: {format}")),
    };
    println!("{output}");
    Ok(())
}

async fn trace(args: Args<'_, '_>, mut client: Client) -> Result<()> {
    client.send(Command::Trace(match args.subcommand() {
        Some(("filter", args)) => Trace::Filter(args.arg("filter")?),
        Some(("print",  args)) => Trace::Print(args.arg("level")?),
        Some(("export", args)) => Trace::Export(args.arg("level")?),
        Some(_) | None         => return Err(anyhow!("unsupported command")),
    })).await
}
