use bulk::Bulk;
use error::Error;
use one_shot::OneShot;
use serve::Serve;

pub mod convert;
pub mod error;
pub mod spec;

pub mod bulk;
pub mod one_shot;
pub mod serve;

#[derive(Debug, clap::Parser)]
pub struct Cli {
    #[clap(flatten)]
    common: CommonArgs,
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    pub async fn run(self) -> Result<(), Error> {
        match self.command {
            Command::Serve(serve) => serve.run(self.common).await,
            Command::OneShot(one_shot) => one_shot.run(self.common).await,
            Command::Bulk(bulk) => bulk.run(self.common).await,
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct CommonArgs {
    // TODO add common arguments as they come up
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Run the server
    Serve(Serve),
    /// Do a single conversion
    OneShot(OneShot),
    /// Do a bulk conversion
    Bulk(Bulk),
}
