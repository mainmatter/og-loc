use std::net::SocketAddr;

use axum::{
    body::Body,
    extract::Path,
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use tokio::net::TcpListener;

use crate::{convert::CrateData, error::Error, spec::CrateName, CommonArgs};

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

        let app = Router::new().route("/og/:name", get(og));

        let listener = TcpListener::bind(self.addr).await?;

        axum::serve(listener, app.into_make_service()).await?;

        Ok(())
    }
}
