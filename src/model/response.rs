use super::{FetcherRateLimit, Repository};

/// A response containing a list of public repositories and their metadata.
#[derive(Debug)]
pub struct Response {
    /// Retrieved repositories and their metadata
    pub(crate) repositories: Vec<Repository>,

    /// The API rate limit information
    pub(crate) rate_limit: FetcherRateLimit,
}

impl Response {
    /// Creates a new `Response` instance with the given repository metadata.
    pub fn new(repositories: Vec<Repository>, rate_limit: FetcherRateLimit) -> Self {
        Self {
            repositories,
            rate_limit,
        }
    }

    /// Retrieves the list of repositories.
    pub fn repositories(&self) -> &[Repository] {
        &self.repositories
    }

    /// Retrieves the API rate limit information.
    pub fn rate_limit(&self) -> &FetcherRateLimit {
        &self.rate_limit
    }
}
