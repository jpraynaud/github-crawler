use crate::{Repository, StdResult};

/// A trait for persisting repository data to a storage medium.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait RepositoryPersister: Sync + Send {
    /// Persists the repository data to a storage medium.
    async fn persist(&self, data: &[Repository]) -> StdResult<u32>;
}
