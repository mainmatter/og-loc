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
