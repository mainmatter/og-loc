use std::{net::SocketAddr, sync::Arc};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{
        header::{CONTENT_LENGTH, CONTENT_TYPE},
        HeaderMap,
    },
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tokio::net::TcpListener;

use crate::{augment::CrateDb, error::Error, spec::CrateName, CommonArgs};

#[derive(Debug, clap::Args)]
pub struct Serve {
    /// The socket address to listen on
    #[arg(env, long, short, default_value = "127.0.0.1:3000")]
    pub addr: SocketAddr,
}

impl Serve {
    /// Run the [`Serve`] subcommand. Sets up a simple HTTP server that
    /// listens on the configured socket address and exposes the Open
    /// Graph image generation funcationality under the `/og/:name` and
    /// `/og/:name/:version` GET endpoints.
    pub async fn run(self, common: CommonArgs) -> Result<(), Error> {
        let db = CrateDb::preload_all(common.db_dump_path).await?;
        #[axum::debug_handler]
        async fn og(
            Path(spec): Path<CrateName>,
            State(db): State<Arc<CrateDb>>,
        ) -> Result<Response, Error> {
            let data = db.augment_crate_spec(spec)?;
            let png = data.render_as_png().await;

            let mut headers = HeaderMap::new();
            headers.append(CONTENT_TYPE, "image/png".parse().unwrap());
            headers.append(CONTENT_LENGTH, png.len().into());
            let body = Body::from(png);

            Ok((headers, body).into_response())
        }

        let app = Router::new()
            .route("/og/:name", get(og))
            .route("/og/:name/", get(og))
            .with_state(Arc::new(db));

        let listener = TcpListener::bind(self.addr).await?;

        axum::serve(listener, app.into_make_service()).await?;

        Ok(())
    }
}
