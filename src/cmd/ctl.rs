use anyhow::Result;
use crate::args::Args;
use crate::ctl::{Client, Command, Trace};

pub async fn ctl(args: Args<'_, '_>) -> Result<()> {
    let socket = args.arg::<String>("socket")?;
    let client = Client::new(socket).await?;

    match args.subcommand() {
        Some(("trace", args)) => trace(args, client).await,
        _                     => unreachable!(),
    }
}

async fn trace(args: Args<'_, '_>, mut client: Client) -> Result<()> {
    client.send(Command::Trace(match args.subcommand() {
        Some(("filter", args)) => Trace::Filter(args.arg("filter")?),
        Some(("print",  args)) => Trace::Print(args.arg("level")?),
        Some(("export", args)) => Trace::Export(args.arg("level")?),
        _                      => unreachable!(),
    })).await
}
