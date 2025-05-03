use std::sync::Arc;

use chrono::Utc;
use log::warn;
use tokio::time::sleep;

use crate::{RepositoryFetcher, Request, Response, StdResult};

/// This struct is responsible for enforcing rate limits on fetcher requests.
pub struct FetcherRateLimitEnforcer {
    /// The fetcher to be rate limited.
    fetcher: Arc<dyn RepositoryFetcher>,
}

impl FetcherRateLimitEnforcer {
    /// Creates a new `FetcherRateLimitEnforcer` instance with the given fetcher.
    pub fn new(fetcher: Arc<dyn RepositoryFetcher>) -> Self {
        Self { fetcher }
    }
}

#[async_trait::async_trait]
impl RepositoryFetcher for FetcherRateLimitEnforcer {
    /// Enforce the rate limit on the fetcher requests.
    async fn fetch(&self, request: &Request) -> StdResult<Option<(Response, Vec<Request>)>> {
        match self.fetcher.fetch(request).await? {
            Some((response, requests)) => {
                if response.rate_limit().is_exceeded() {
                    let duration_until_reset =
                        response.rate_limit().duration_until_reset(Utc::now())?;
                    warn!(
                        "Fetcher rate limit exceeded for request, waiting for {duration_until_reset:?}"
                    );
                    sleep(duration_until_reset).await;
                }
                Ok(Some((response, requests)))
            }
            None => Ok(None),
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::{FetcherRateLimit, MockRepositoryFetcher};

    use super::*;

    #[tokio::test]
    async fn fetch_rate_limit_not_exceeded() {
        let now = Utc::now();
        let reset_at = now + chrono::Duration::seconds(60);
        let fetcher_rate_limit_enforcer = FetcherRateLimitEnforcer::new(Arc::new({
            let reset_at_clone = reset_at.clone();
            let mut mock_fetcher = MockRepositoryFetcher::new();
            mock_fetcher
                .expect_fetch()
                .returning(move |_| {
                    Ok(Some((
                        Response::new(
                            vec![],
                            FetcherRateLimit {
                                limit: 1000,
                                remaining: 100,
                                cost: 1,
                                reset_at: reset_at_clone.to_rfc3339(),
                            },
                        ),
                        vec![],
                    )))
                })
                .times(1);

            mock_fetcher
        }));
        let request = Request::dummy_search_organization();

        fetcher_rate_limit_enforcer.fetch(&request).await.unwrap();

        assert!(reset_at > Utc::now());
    }

    #[tokio::test]
    async fn fetch_rate_limit_exceeded() {
        let now = Utc::now();
        let reset_at = now + chrono::Duration::seconds(1);
        let fetcher_rate_limit_enforcer = FetcherRateLimitEnforcer::new(Arc::new({
            let reset_at_clone = reset_at.clone();
            let mut mock_fetcher = MockRepositoryFetcher::new();
            mock_fetcher
                .expect_fetch()
                .returning(move |_| {
                    Ok(Some((
                        Response::new(
                            vec![],
                            FetcherRateLimit {
                                limit: 1000,
                                remaining: 0,
                                cost: 1,
                                reset_at: reset_at_clone.to_rfc3339(),
                            },
                        ),
                        vec![],
                    )))
                })
                .times(1);

            mock_fetcher
        }));
        let request = Request::dummy_search_organization();

        fetcher_rate_limit_enforcer.fetch(&request).await.unwrap();

        assert!(reset_at <= Utc::now());
    }
}
