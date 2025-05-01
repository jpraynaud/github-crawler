use crate::{Request, Response, StdResult};

/// A trait for fetching repository data from the API.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait RepositoryFetcher: Sync + Send {
    /// Fetches the repositories data from the API.
    async fn fetch(&self, request: &Request) -> StdResult<Option<(Response, Vec<Request>)>>;
}
