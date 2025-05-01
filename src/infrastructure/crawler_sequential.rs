use std::{collections::BinaryHeap, sync::Arc};

use anyhow::anyhow;
use log::{debug, info, warn};

use crate::{
    FetcherRateLimit, RepositoryCrawler, RepositoryFetcher, RepositoryPersister, Request, StdResult,
};

/// A sequential crawler
pub struct SequentialCrawler {
    fetcher: Arc<dyn RepositoryFetcher>,
    persister: Arc<dyn RepositoryPersister>,
}

impl SequentialCrawler {
    /// Creates a new `SequentialCrawler` instance with the given fetcher and persister.
    pub fn new(
        fetcher: Arc<dyn RepositoryFetcher>,
        persister: Arc<dyn RepositoryPersister>,
    ) -> Self {
        Self { fetcher, persister }
    }
}

#[async_trait::async_trait]
impl RepositoryCrawler for SequentialCrawler {
    async fn crawl(&self, requests: Vec<Request>, total_repositories: u32) -> StdResult<()> {
        if requests.len() == 0 {
            return Err(anyhow!(
                "Not enough requests to process, at least one request is required"
            ));
        }

        let mut priority_queue = BinaryHeap::from(requests);
        let mut total_fetcher_calls = 0;
        let mut api_rate_limit: FetcherRateLimit = FetcherRateLimit::default();
        let mut total_persisted_repositories = 0;
        let mut total_collisions_repositories = 0;

        while let Some(request) = priority_queue.pop() {
            info!("Processing request: {request}");
            total_fetcher_calls += 1;
            match self.fetcher.fetch(&request).await? {
                Some((response, next_requests)) => {
                    api_rate_limit = response.rate_limit().to_owned();
                    let repositories = response.repositories();
                    if repositories.is_empty() {
                        debug!("No repositories found for request: {request:?}");
                    }
                    for repository in repositories {
                        info!("Fetched {repository}");
                    }
                    for next_request in next_requests {
                        priority_queue.push(next_request);
                    }
                    let total_persisted_repositories_call =
                        self.persister.persist(repositories).await?;

                    total_persisted_repositories += total_persisted_repositories_call;
                    total_collisions_repositories +=
                        repositories.len() as u32 - total_persisted_repositories_call;
                    if total_persisted_repositories >= total_repositories {
                        break;
                    }
                }
                None => {}
            }

            warn!(
                "Persisted repositories: done={total_persisted_repositories}/{total_repositories}, coll={total_collisions_repositories}, Requests: done={total_fetcher_calls}, buff={}, {api_rate_limit}",
                priority_queue.len()
            );
        }

        if total_persisted_repositories < total_repositories {
            return Err(anyhow::anyhow!(
                "Not enough repositories crawled. Expected: {total_repositories}, crawled: {total_persisted_repositories}"
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use mockall::predicate::eq;

    use crate::{
        FetcherRateLimit, MockRepositoryFetcher, MockRepositoryPersister, Repository, Response,
    };

    use super::*;

    #[tokio::test]
    async fn crawler_fails_if_not_enough_requests() {
        let fetcher = MockRepositoryFetcher::new();
        let persister = MockRepositoryPersister::new();
        let crawler = SequentialCrawler::new(Arc::new(fetcher), Arc::new(persister));

        crawler
            .crawl(vec![], 0)
            .await
            .expect_err("Crawler should fail if not enough requests");
    }

    #[tokio::test]
    async fn crawler_fails_if_not_enough_repositories_are_persisted() {
        let fetcher = {
            let mut fetcher = MockRepositoryFetcher::new();
            fetcher
                .expect_fetch()
                .returning(|_| {
                    Ok(Some((
                        Response::new(
                            vec![
                                Repository::new("repository-1", "org-1", 10),
                                Repository::new("repository-2", "org-2", 20),
                            ],
                            FetcherRateLimit::dummy(),
                        ),
                        vec![],
                    )))
                })
                .times(1);

            fetcher
        };
        let persister = {
            let mut persister = MockRepositoryPersister::new();
            persister.expect_persist().returning(|_| Ok(1)).times(1);

            persister
        };
        let requests = vec![Request::dummy_search_organization()];
        let crawler = SequentialCrawler::new(Arc::new(fetcher), Arc::new(persister));

        crawler
            .crawl(requests, 10)
            .await
            .expect_err("Crawler should fail if not enough persisted repositories");
    }

    #[tokio::test]
    async fn crawler_fails_if_fetch_task_fails() {
        let fetcher = {
            let mut fetcher = MockRepositoryFetcher::new();
            fetcher
                .expect_fetch()
                .returning(|_| Err(anyhow!("Error fetching data")))
                .times(1);

            fetcher
        };
        let persister = MockRepositoryPersister::new();
        let requests = vec![Request::dummy_search_organization()];
        let crawler = SequentialCrawler::new(Arc::new(fetcher), Arc::new(persister));

        crawler
            .crawl(requests, 1)
            .await
            .expect_err("Crawler should fail if fetch task fails");
    }

    #[tokio::test]
    async fn crawler_fails_if_persist_task_fails() {
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
        let persister = {
            let mut persister = MockRepositoryPersister::new();
            persister
                .expect_persist()
                .returning(|_| Err(anyhow!("Error persisting data")))
                .times(1);

            persister
        };
        let requests = vec![Request::dummy_search_organization()];
        let crawler = SequentialCrawler::new(Arc::new(fetcher), Arc::new(persister));

        crawler
            .crawl(requests, 1)
            .await
            .expect_err("Crawler should fail if one persist task fails");
    }

    #[tokio::test]
    async fn crawler_success() {
        let fetcher = {
            let mut fetcher = MockRepositoryFetcher::new();
            fetcher
                .expect_fetch()
                .returning(|_| {
                    Ok(Some((
                        Response::new(
                            vec![
                                Repository::new("repository-1", "org-1", 10),
                                Repository::new("repository-2", "org-2", 20),
                            ],
                            FetcherRateLimit::dummy(),
                        ),
                        vec![Request::dummy_search_organization()],
                    )))
                })
                .times(1);
            fetcher
                .expect_fetch()
                .returning(|_| {
                    Ok(Some((
                        Response::new(
                            vec![
                                Repository::new("repository-2", "org-2", 20),
                                Repository::new("repository-3", "org-3", 30),
                            ],
                            FetcherRateLimit::dummy(),
                        ),
                        vec![],
                    )))
                })
                .times(1);

            fetcher
        };
        let persister = {
            let mut persister = MockRepositoryPersister::new();
            persister
                .expect_persist()
                .with(eq(vec![
                    Repository::new("repository-1", "org-1", 10),
                    Repository::new("repository-2", "org-2", 20),
                ]))
                .returning(|_| Ok(2))
                .times(1);
            persister
                .expect_persist()
                .with(eq(vec![
                    Repository::new("repository-2", "org-2", 20),
                    Repository::new("repository-3", "org-3", 30),
                ]))
                .returning(|_| Ok(1))
                .times(1);

            persister
        };
        let requests = vec![Request::dummy_search_organization()];
        let crawler = SequentialCrawler::new(Arc::new(fetcher), Arc::new(persister));

        crawler.crawl(requests, 3).await.unwrap();
    }
}
