use std::{net::SocketAddr, path::PathBuf};

use convert::CrateData;

use error::Error;
use spec::CrateVersionSpec;
use tokio::io::AsyncWriteExt;

pub mod convert;
pub mod error;
mod spec;

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
}

#[derive(Debug, clap::Args)]
pub struct Serve {
    /// The socket address to listen on
    #[arg(env, long, short)]
    pub addr: SocketAddr,
}

impl Serve {
    pub async fn run(self, _common: CommonArgs) -> Result<(), Error> {
        todo!("Setup http server that serves png's given a path /og/:crate_name/:version")
    }
}

#[derive(Debug, clap::Args)]
pub struct OneShot {
    #[clap(flatten)]
    pub spec: CrateVersionSpec,
    /// The path to the PNG output file
    #[arg(env, long = "out", short)]
    pub out_path: PathBuf,
}

impl OneShot {
    pub async fn run(self, _common: CommonArgs) -> Result<(), Error> {
        let data = CrateData::augment_crate_version_spec(self.spec).await?;
        let png = data.render_as_png();
        let mut out_file = tokio::fs::File::create(self.out_path).await?;
        out_file.write_all(&png).await?;

        Ok(())
    }
}
