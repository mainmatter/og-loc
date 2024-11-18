use std::{net::SocketAddr, path::PathBuf};

use axum::{
    body::Body,
    extract::Path,
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use convert::CrateData;

use error::Error;
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use spec::CrateName;
use tokio::{io::AsyncWriteExt, net::TcpListener};

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
    /// Run the [`Serve`] subcommand. Sets up a simple HTTP server that
    /// listens on the configured socket address and exposes the Open
    /// Graph image generation funcationality under the `/og/:name` and
    /// `/og/:name/:version` GET endpoints.
    pub async fn run(self, _common: CommonArgs) -> Result<(), Error> {
        #[axum::debug_handler]
        async fn og(Path(spec): Path<CrateName>) -> Result<Response, Error> {
            let data = CrateData::augment_crate_version_spec(spec).await?;
            let png = data.render_as_png();

            let mut headers = HeaderMap::new();
            headers.append(CONTENT_TYPE, "image/png".parse().unwrap());
            headers.append(CONTENT_LENGTH, png.len().into());
            let body = Body::from(png);

            Ok((headers, body).into_response())
        }

        let app = Router::new()
            .route("/og/:name", get(og));

        let listener = TcpListener::bind(self.addr).await?;

        axum::serve(listener, app.into_make_service()).await?;

        Ok(())
    }
}

#[derive(Debug, clap::Args)]
pub struct OneShot {
    /// The name of the crate
    #[arg(env, long, short)]
    pub name: CrateName,
    /// The path to the PNG output file
    #[arg(env, long = "out", short)]
    pub out_path: PathBuf,
}

impl OneShot {
    pub async fn run(self, _common: CommonArgs) -> Result<(), Error> {
        let data = CrateData::augment_crate_version_spec(self.name).await?;
        let png = data.render_as_png();
        let mut out_file = tokio::fs::File::create(self.out_path).await?;
        out_file.write_all(&png).await?;

        Ok(())
    }
}
