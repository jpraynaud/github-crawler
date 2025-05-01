use crate::{Request, StdResult};

/// A trait for retrieving GitHub repositories and associated metadata.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait RepositoryCrawler {
    /// Crawl the GitHub API for repositories.
    async fn crawl(&self, requests: Vec<Request>, total_repositories: u32) -> StdResult<()>;
}
