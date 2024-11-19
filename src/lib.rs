use std::path::PathBuf;

use bulk::Bulk;
use error::Error;
use one_shot::OneShot;
use serve::Serve;

pub mod augment;
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
    /// The path of the database dump
    #[arg(short, long, env, default_value = "./db-dump.tar.gz")]
    db_dump_path: PathBuf,
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
