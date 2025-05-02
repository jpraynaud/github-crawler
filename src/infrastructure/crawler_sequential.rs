use std::{
    collections::{BinaryHeap, HashSet},
    sync::Arc,
};

use anyhow::anyhow;
use log::{info, warn};
use tokio::sync::Mutex;

use crate::{
    FetcherRateLimit, RepositoryCrawler, RepositoryFetcher, RepositoryPersister, Request, Response,
    StdResult,
};

/// A state for the sequential crawler
#[derive(Debug, Default)]
pub struct SequentialCrawlerState {
    requests_priority_queue: BinaryHeap<Request>,
    requests_pushed: HashSet<Request>,
    pub(super) total_fetcher_calls: u32,
    pub(super) api_rate_limit: FetcherRateLimit,
    pub(super) total_persisted_repositories: u32,
    pub(super) total_collisions_repositories: u32,
}

impl SequentialCrawlerState {
    /// Pushes a request to the priority queue if it hasn't been pushed before.
    pub fn push_request(&mut self, request: Request) {
        if self.requests_pushed.contains(&request) {
            info!("Request already pushed: {request}");
            return;
        }
        self.requests_pushed.insert(request.clone());
        self.requests_priority_queue.push(request);
    }

    /// Pushes multiple requests to the priority queue.
    pub fn push_requests(&mut self, requests: Vec<Request>) {
        for request in requests {
            self.push_request(request);
        }
    }

    /// Pops a request from the priority queue.
    pub fn pop_request(&mut self) -> Option<Request> {
        self.requests_priority_queue.pop()
    }
}

/// A sequential crawler
pub struct SequentialCrawler {
    fetcher: Arc<dyn RepositoryFetcher>,
    persister: Arc<dyn RepositoryPersister>,
    state: Arc<Mutex<SequentialCrawlerState>>,
}

impl SequentialCrawler {
    /// Creates a new `SequentialCrawler` instance with the given fetcher and persister.
    pub fn new(
        fetcher: Arc<dyn RepositoryFetcher>,
        persister: Arc<dyn RepositoryPersister>,
    ) -> Self {
        Self {
            fetcher,
            persister,
            state: Arc::new(Mutex::new(SequentialCrawlerState::default())),
        }
    }

    async fn process_response(
        &self,
        response: &Response,
        request: &Request,
        state: &mut SequentialCrawlerState,
    ) -> StdResult<()> {
        state.api_rate_limit = response.rate_limit().to_owned();
        let repositories = response.repositories();
        if repositories.is_empty() {
            info!("No repositories found for request: {request:?}");
        }
        for repository in repositories {
            info!("Fetched {repository}");
        }
        let total_persisted_repositories_call = self.persister.persist(repositories).await?;
        state.total_persisted_repositories += total_persisted_repositories_call;
        state.total_collisions_repositories +=
            repositories.len() as u32 - total_persisted_repositories_call;

        Ok(())
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

        let mut state = self.state.lock().await;
        state.push_requests(requests);
        while let Some(request) = state.pop_request() {
            info!("Processing request: {request}");
            state.total_fetcher_calls += 1;
            match self.fetcher.fetch(&request).await? {
                Some((response, next_requests)) => {
                    self.process_response(&response, &request, &mut state)
                        .await?;
                    state.push_requests(next_requests);
                    if state.total_persisted_repositories >= total_repositories {
                        break;
                    }
                }
                None => {}
            }
            state.push_request(request);

            warn!(
                "Persisted repositories: done={}/{total_repositories}, collisions={}, Requests: done={}, buffered={}, {}",
                state.total_persisted_repositories,
                state.total_collisions_repositories,
                state.total_fetcher_calls,
                state.requests_priority_queue.len(),
                state.api_rate_limit
            );
        }

        if state.total_persisted_repositories < total_repositories {
            return Err(anyhow::anyhow!(
                "Not enough repositories crawled. Expected: {total_repositories}, crawled: {}",
                state.total_persisted_repositories
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
        let crawler = SequentialCrawler::new(Arc::new(fetcher), Arc::new(persister));

        crawler.crawl(requests, 3).await.unwrap();
    }
}
