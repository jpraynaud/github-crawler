use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use log::warn;
use tokio::time::sleep;

use crate::{Repository, RepositoryPersister, StdResult};

/// A struct that retries a RepositoryPersister a specified number of times in case of failure with exponential backoff strategy.
pub struct PersisterRetrier {
    /// The persister to be retried.
    persister: Arc<dyn RepositoryPersister>,
    /// The maximum number of retries for a request.
    max_retries: u32,
    /// The base delay for exponential backoff.
    base_delay: Duration,
}

impl PersisterRetrier {
    /// Creates a new `PersisterRetrier` instance with the given maximum number of retries.
    pub fn new(
        persister: Arc<dyn RepositoryPersister>,
        max_retries: u32,
        base_delay: Duration,
    ) -> Self {
        Self {
            persister,
            max_retries,
            base_delay,
        }
    }

    fn calculate_exponential_backoff_delay(&self, attempt: u32) -> Duration {
        self.base_delay * (2u32.pow(attempt.min(31)))
    }
}

#[async_trait::async_trait]
impl RepositoryPersister for PersisterRetrier {
    /// Retries the persist operation if it fails, up to the maximum number of retries.
    async fn persist(&self, repositories: &[Repository]) -> StdResult<u32> {
        let mut attempts = 0;

        loop {
            match self.persister.persist(repositories).await {
                Ok(res) => return Ok(res),
                Err(e) => {
                    warn!("Persist attempt #{} failed: {}", attempts + 1, e);
                    attempts += 1;
                    if attempts >= self.max_retries {
                        return Err(anyhow!("Failed after {} attempts: {}", attempts, e));
                    }
                    sleep(self.calculate_exponential_backoff_delay(attempts)).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MockRepositoryPersister, Repository};
    use std::sync::Arc;

    #[tokio::test]
    async fn persist_success_on_first_attempt() {
        let persister = {
            let mut persister = MockRepositoryPersister::new();
            persister.expect_persist().returning(|_| Ok(10)).times(1);

            persister
        };
        let retrier = PersisterRetrier::new(Arc::new(persister), 3, Duration::from_millis(10));

        let result = retrier
            .persist(&[Repository::new("repository-1", "org-1", 100)])
            .await;
        assert_eq!(result.unwrap(), 10);
    }

    #[tokio::test]
    async fn persist_success_after_retries() {
        let persister = {
            let mut persister = MockRepositoryPersister::new();
            persister
                .expect_persist()
                .returning(|_| Err(anyhow!("Temporary failure")))
                .times(2);
            persister.expect_persist().returning(|_| Ok(10)).times(1);

            persister
        };
        let retrier = PersisterRetrier::new(Arc::new(persister), 3, Duration::from_millis(10));

        let result = retrier
            .persist(&[Repository::new("repository-1", "org-1", 100)])
            .await
            .unwrap();
        assert_eq!(result, 10);
    }

    #[tokio::test]
    async fn persist_failure_after_max_retries() {
        let persister = {
            let mut persister = MockRepositoryPersister::new();
            persister
                .expect_persist()
                .returning(|_| Err(anyhow!("Temporary failure")))
                .times(3);

            persister
        };
        let retrier = PersisterRetrier::new(Arc::new(persister), 3, Duration::from_millis(10));

        retrier
            .persist(&[Repository::new("repository-1", "org-1", 100)])
            .await
            .expect_err("Should retrurn an error after max retries");
    }
}
