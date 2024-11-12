use axum::response::IntoResponse;
use reqwest::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Http client error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("No crate with that name and version was found")]
    NotFound,

    #[error("That's not a valid crate name: {0}")]
    InvalidCrateName(#[from] crate::spec::InvalidCrateName),

    #[error("That's not a valid semver version identifier: {0}")]
    InavlidCrateVersion(#[from] semver::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Error: {0}")]
    Other(#[from] anyhow::Error),
}

impl Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Error::Http(_) => StatusCode::SERVICE_UNAVAILABLE,
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::InvalidCrateName(_) => StatusCode::BAD_REQUEST,
            Error::InavlidCrateVersion(_) => StatusCode::BAD_REQUEST,
            Error::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        (self.status_code(), self.to_string()).into_response()
    }
}
