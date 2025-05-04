use std::sync::Arc;

use anyhow::anyhow;
use log::{info, warn};

use crate::{
    CrawlerState, RepositoryCrawler, RepositoryFetcher, RepositoryPersister, Request, Response,
    StdResult,
};

/// A worker crawler
pub struct WorkerCrawler {
    fetcher: Arc<dyn RepositoryFetcher>,
    persister: Arc<dyn RepositoryPersister>,
    state: Arc<CrawlerState>,
}

impl WorkerCrawler {
    /// Creates a new `WorkerCrawler` instance with the given fetcher and persister.
    pub fn new(
        fetcher: Arc<dyn RepositoryFetcher>,
        persister: Arc<dyn RepositoryPersister>,
        state: Arc<CrawlerState>,
    ) -> Self {
        Self {
            fetcher,
            persister,
            state,
        }
    }

    async fn process_response(&self, response: &Response, request: &Request) -> StdResult<()> {
        self.state
            .update_current_api_rate_limit(response.rate_limit().to_owned())
            .await;
        let repositories = response.repositories();
        if repositories.is_empty() {
            info!("No repositories found for request: {request:?}");
        }
        for repository in repositories {
            info!("Fetched {repository}");
        }
        let total_persisted_repositories_call = self.persister.persist(repositories).await?;
        self.state
            .increment_total_persisted_repositories(total_persisted_repositories_call)
            .await;
        self.state
            .increment_total_collisions_repositories(
                repositories.len() as u32 - total_persisted_repositories_call,
            )
            .await;

        Ok(())
    }
}

#[async_trait::async_trait]
impl RepositoryCrawler for WorkerCrawler {
    async fn crawl(&self, requests: Vec<Request>, total_repositories: u32) -> StdResult<()> {
        if requests.len() == 0 {
            return Err(anyhow!(
                "Not enough requests to process, at least one request is required"
            ));
        }
        self.state
            .set_total_repositories_target(total_repositories)
            .await;
        self.state.push_requests(requests).await;
        while !self.state.has_completed().await? {
            if let Some(request) = self.state.pop_request().await {
                info!("Processing request: {request}");
                self.state.increment_total_fetcher_calls(1).await;
                match self.fetcher.fetch(&request).await? {
                    Some((response, next_requests)) => {
                        self.process_response(&response, &request).await?;
                        self.state.push_requests(next_requests).await;
                    }
                    None => {}
                }
                self.state.push_request(request).await;
                warn!("{}", self.state.state_summary().await);
            }
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
        let crawler = WorkerCrawler::new(
            Arc::new(fetcher),
            Arc::new(persister),
            Arc::new(CrawlerState::default()),
        );

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
        let crawler = WorkerCrawler::new(
            Arc::new(fetcher),
            Arc::new(persister),
            Arc::new(CrawlerState::default()),
        );

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
        let crawler = WorkerCrawler::new(
            Arc::new(fetcher),
            Arc::new(persister),
            Arc::new(CrawlerState::default()),
        );

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
        let crawler = WorkerCrawler::new(
            Arc::new(fetcher),
            Arc::new(persister),
            Arc::new(CrawlerState::default()),
        );

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
                        vec![Request::SearchOrganization(
                            crate::SearchOrganizationRequest::new(
                                "org-1",
                                10,
                                Some("after".to_string()),
                            ),
                        )],
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
        let requests = vec![Request::SearchOrganization(
            crate::SearchOrganizationRequest::new("org-1", 10, None),
        )];
        let crawler = WorkerCrawler::new(
            Arc::new(fetcher),
            Arc::new(persister),
            Arc::new(CrawlerState::default()),
        );

        crawler.crawl(requests, 3).await.unwrap();
    }
}
