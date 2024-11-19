use axum::{http::StatusCode, response::IntoResponse};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No crate with that name was found")]
    NotFound,

    #[error("That's not a valid crate name: {0}")]
    InvalidCrateName(#[from] crate::spec::InvalidCrateName),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("DB dump load error: {0}")]
    DbDump(#[from] db_dump::Error),

    #[error("Error: {0}")]
    Other(#[from] anyhow::Error),

    #[error("Buld input error: {0}")]
    BulkInput(#[from] crate::bulk::BulkInputError),
}

impl Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::InvalidCrateName(_) => StatusCode::BAD_REQUEST,
            Error::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::BulkInput(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::DbDump(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        (self.status_code(), self.to_string()).into_response()
    }
}
