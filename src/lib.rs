use std::{path::PathBuf, sync::LazyLock};

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

/// Set up a reusable HTTP client with a User Agent
/// that allows for identifying this application.
pub static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    const CARGO_PKG_NAME: &str = env!("CARGO_PKG_NAME");
    const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
    const CARGO_PKG_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
    reqwest::ClientBuilder::new()
        .user_agent(format!(
            "{CARGO_PKG_NAME}/{CARGO_PKG_VERSION} ({CARGO_PKG_REPOSITORY})"
        ))
        .build()
        .unwrap()
});
