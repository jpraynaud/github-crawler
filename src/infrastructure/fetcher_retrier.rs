use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use log::warn;
use tokio::time::sleep;

use crate::{CrawlerState, RepositoryFetcher, Request, Response, StdResult};

/// A struct that retries a RepositoryFetcher a specified number of times in case of failure with exponential backoff strategy.
pub struct FetcherRetrier {
    /// The fetcher to be retried.
    fetcher: Arc<dyn RepositoryFetcher>,

    /// The maximum number of retries for a request.
    max_retries: u32,

    /// The base delay for exponential backoff.
    base_delay: Duration,

    /// The state of the crawler
    state: Arc<CrawlerState>,
}

impl FetcherRetrier {
    /// Creates a new `FetcherRetrier` instance with the given maximum number of retries.
    pub fn new(
        fetcher: Arc<dyn RepositoryFetcher>,
        max_retries: u32,
        base_delay: Duration,
        state: Arc<CrawlerState>,
    ) -> Self {
        Self {
            fetcher,
            max_retries,
            base_delay,
            state,
        }
    }

    fn calculate_exponential_backoff_delay(&self, attempt: u32) -> Duration {
        self.base_delay * (2u32.pow(attempt.min(31)))
    }
}

#[async_trait::async_trait]
impl RepositoryFetcher for FetcherRetrier {
    /// Retries the request if it fails, up to the maximum number of retries.
    async fn fetch(&self, request: &Request) -> StdResult<Option<(Response, Vec<Request>)>> {
        let mut attempts = 0;

        while !self.state.has_completed().await? {
            match self.fetcher.fetch(request).await {
                Ok(res) => return Ok(res),
                Err(e) => {
                    warn!("Fetch attempt #{} failed: {}", attempts + 1, e);
                    attempts += 1;
                    if attempts >= self.max_retries {
                        return Err(anyhow!("Failed after {} attempts: {}", attempts, e));
                    }
                    sleep(self.calculate_exponential_backoff_delay(attempts)).await;
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use crate::{FetcherRateLimit, MockRepositoryFetcher, Repository};

    use super::*;

    #[tokio::test]
    async fn fetch_success_on_first_attempt() {
        let state = {
            let state = CrawlerState::default();
            state.set_total_repositories_target(10).await;
            state
                .push_request(Request::dummy_search_organization())
                .await;

            state
        };
        let fetcher = {
            let mut fetcher = MockRepositoryFetcher::new();
            fetcher
                .expect_fetch()
                .returning(|_| {
                    Ok(Some((
                        Response::new(
                            vec![Repository::new("repository-1", "org-1", 10)],
                            FetcherRateLimit::dummy(),
                        ),
                        vec![],
                    )))
                })
                .times(1);

            fetcher
        };
        let retrier = FetcherRetrier::new(
            Arc::new(fetcher),
            3,
            Duration::from_millis(10),
            Arc::new(state),
        );

        retrier
            .fetch(&Request::dummy_search_organization())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn fetch_success_after_retries() {
        let state = {
            let state = CrawlerState::default();
            state.set_total_repositories_target(10).await;
            state
                .push_request(Request::dummy_search_organization())
                .await;

            state
        };
        let fetcher = {
            let mut fetcher = MockRepositoryFetcher::new();
            fetcher
                .expect_fetch()
                .returning(|_| Err(anyhow!("Error fetching data")))
                .times(2);
            fetcher
                .expect_fetch()
                .returning(|_| {
                    Ok(Some((
                        Response::new(
                            vec![Repository::new("repository-1", "org-1", 10)],
                            FetcherRateLimit::dummy(),
                        ),
                        vec![],
                    )))
                })
                .times(1);

            fetcher
        };
        let retrier = FetcherRetrier::new(
            Arc::new(fetcher),
            3,
            Duration::from_millis(10),
            Arc::new(state),
        );

        retrier
            .fetch(&Request::dummy_search_organization())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn fetch_failure_after_max_retries() {
        let state = {
            let state = CrawlerState::default();
            state.set_total_repositories_target(10).await;
            state
                .push_request(Request::dummy_search_organization())
                .await;

            state
        };
        let fetcher = {
            let mut fetcher = MockRepositoryFetcher::new();
            fetcher
                .expect_fetch()
                .returning(|_| Err(anyhow!("Error fetching data")))
                .times(3);

            fetcher
        };
        let retrier = FetcherRetrier::new(
            Arc::new(fetcher),
            3,
            Duration::from_millis(10),
            Arc::new(state),
        );

        retrier
            .fetch(&Request::dummy_search_organization())
            .await
            .expect_err("Expected failure after max retries");
    }
}
