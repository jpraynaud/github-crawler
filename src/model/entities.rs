use std::{
    collections::{BinaryHeap, HashSet},
    fmt::Display,
    ops::Deref,
};

use log::info;
use serde::Deserialize;
use tokio::sync::RwLock;

use super::{Request, StdResult};

/// The name of a repository.
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct RepositoryName(pub String);

impl Deref for RepositoryName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for RepositoryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The name of an organization.
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct OrganizationName(pub String);

impl Deref for OrganizationName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for OrganizationName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The number of stars a repository has.
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct StarsCounter(pub u32);

impl Deref for StarsCounter {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for StarsCounter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
/// Metadata of a GitHub repository.
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct Repository {
    /// The name of the repository.
    repository_name: RepositoryName,

    /// The name of the organization that owns the repository.
    organization_name: OrganizationName,

    /// The number of stars the repository has.
    total_stars: StarsCounter,
}

impl Repository {
    /// Creates a new `Repository` instance.
    pub fn new(repository_name: &str, organization_name: &str, total_stars: u32) -> Self {
        Self {
            repository_name: RepositoryName(repository_name.to_string()),
            organization_name: OrganizationName(organization_name.to_string()),
            total_stars: StarsCounter(total_stars),
        }
    }

    /// Retrieves the repository name.
    pub fn repository_name(&self) -> &RepositoryName {
        &self.repository_name
    }

    /// Retrieves the organization name.
    pub fn organization_name(&self) -> &OrganizationName {
        &self.organization_name
    }

    /// Retrieves the total stars of the repository.
    pub fn total_stars(&self) -> &StarsCounter {
        &self.total_stars
    }
}

impl Display for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Repository: {}, Organization: {}, Stars: {}",
            self.repository_name, self.organization_name, self.total_stars
        )
    }
}

/// A state for the sequential crawler
#[derive(Debug, Default)]
pub struct CrawlerState {
    /// A priority queue for requests to be processed
    requests_priority_queue: RwLock<BinaryHeap<Request>>,

    /// A set of requests that have already been pushed to the queue to avoid duplicates
    requests_pushed: RwLock<HashSet<Request>>,

    /// The total number of repositories to be fetched
    total_repositories_target: RwLock<u32>,

    /// The total number of fetcher calls made
    total_fetcher_calls: RwLock<u32>,

    /// The total number of repositories persisted
    total_persisted_repositories: RwLock<u32>,

    /// The total number of collisions (repositories that were not persisted because they already exist)
    total_collisions_repositories: RwLock<u32>,

    /// The API rate limit for the fetchers
    current_api_rate_limit: RwLock<FetcherRateLimit>,
}

impl CrawlerState {
    pub async fn has_completed(&self) -> StdResult<bool> {
        let total_repositories_target = self.get_total_repositories_target().await;
        let total_persisted_repositories = self.get_total_persisted_repositories().await;
        let has_persisted_enough_repositories =
            total_persisted_repositories >= total_repositories_target;

        if has_persisted_enough_repositories {
            Ok(true)
        } else {
            let has_empty_priority_queue = {
                let requests_priority_queue = self.requests_priority_queue.read().await;
                (*requests_priority_queue).is_empty()
            };
            let has_pushed_requests = {
                let requests_pushed = self.requests_pushed.read().await;
                !(*requests_pushed).is_empty()
            };
            let has_failed = has_empty_priority_queue
                && has_pushed_requests
                && !has_persisted_enough_repositories;
            if has_failed {
                Err(anyhow::anyhow!(
                    "Not enough repositories persisted. Expected: {total_repositories_target}, persisted: {total_persisted_repositories}"
                ))
            } else {
                Ok(false)
            }
        }
    }

    /// Pushes a request to the priority queue if it hasn't been pushed before.
    pub async fn push_request(&self, request: Request) {
        {
            let mut requests_pushed = self.requests_pushed.write().await;
            if (*requests_pushed).contains(&request) {
                info!("Request already pushed: {request}");
                return;
            }
            (*requests_pushed).insert(request.clone());
        }
        let mut requests_priority_queue = self.requests_priority_queue.write().await;
        (*requests_priority_queue).push(request);
    }

    /// Pushes multiple requests to the priority queue.
    pub async fn push_requests(&self, requests: Vec<Request>) {
        for request in requests {
            self.push_request(request).await;
        }
    }

    /// Pops a request from the priority queue.
    pub async fn pop_request(&self) -> Option<Request> {
        let mut requests_priority_queue = self.requests_priority_queue.write().await;

        (*requests_priority_queue).pop()
    }

    /// Sets the total number of repositories to be fetched.
    pub async fn set_total_repositories_target(&self, total_repositories: u32) {
        let mut total_repositories_target = self.total_repositories_target.write().await;
        *total_repositories_target = total_repositories;
    }

    /// Retrieves the total number of repositories to be fetched.
    pub async fn get_total_repositories_target(&self) -> u32 {
        let total_repositories_target = self.total_repositories_target.read().await;
        *total_repositories_target
    }

    /// Increments the total number of persisted repositories.
    pub async fn increment_total_persisted_repositories(&self, increment: u32) {
        let mut total_persisted_repositories = self.total_persisted_repositories.write().await;
        *total_persisted_repositories += increment;
    }

    /// Retrieves the total number of persisted repositories.
    pub async fn get_total_persisted_repositories(&self) -> u32 {
        let total_persisted_repositories = self.total_persisted_repositories.read().await;
        *total_persisted_repositories
    }

    /// Increments the total number of collisions.
    pub async fn increment_total_collisions_repositories(&self, increment: u32) {
        let mut total_collisions_repositories = self.total_collisions_repositories.write().await;
        *total_collisions_repositories += increment;
    }

    /// Retrieves the total number of collisions.
    pub async fn get_total_collisions_repositories(&self) -> u32 {
        let total_collisions_repositories = self.total_collisions_repositories.read().await;
        *total_collisions_repositories
    }

    /// Increments the total number of fetcher calls.
    pub async fn increment_total_fetcher_calls(&self, increment: u32) {
        let mut total_fetcher_calls = self.total_fetcher_calls.write().await;
        *total_fetcher_calls += increment;
    }

    /// Retrieves the total number of fetcher calls.
    pub async fn get_total_fetcher_calls(&self) -> u32 {
        let total_fetcher_calls = self.total_fetcher_calls.read().await;
        *total_fetcher_calls
    }

    /// Updates the API rate limit.
    pub async fn update_current_api_rate_limit(&self, rate_limit: FetcherRateLimit) {
        let mut api_rate_limit = self.current_api_rate_limit.write().await;
        *api_rate_limit = rate_limit;
    }

    /// Retrieves the current API rate limit.
    pub async fn get_current_api_rate_limit(&self) -> FetcherRateLimit {
        let api_rate_limit = self.current_api_rate_limit.read().await;
        api_rate_limit.to_owned()
    }

    /// Returns the summary of the state.
    pub async fn state_summary(&self) -> String {
        let total_fetcher_calls = self.total_fetcher_calls.read().await;
        let total_persisted_repositories = self.total_persisted_repositories.read().await;
        let total_collisions_repositories = self.total_collisions_repositories.read().await;
        let current_api_rate_limit = self.current_api_rate_limit.read().await;
        let total_buffered_requests = self.requests_priority_queue.read().await.len();
        let total_repositories_target = self.get_total_repositories_target().await;

        format!(
            "Repositories: done={total_persisted_repositories}/{total_repositories_target}, collisions={total_collisions_repositories}, Requests: done={total_fetcher_calls}, buffered={total_buffered_requests}, {current_api_rate_limit}",
        )
    }
}

/// A fetcher API rate limit
#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct FetcherRateLimit {
    /// The maximum number of requests that can be made in a given time period.
    pub limit: i32,
    /// The cost of the current request.
    pub cost: i32,
    /// The remaining number of requests that can be made in the current time period.
    pub remaining: i32,
    /// The time at which the rate limit will reset.
    pub reset_at: String,
}

impl FetcherRateLimit {
    #[cfg(test)]
    /// Creates a dummy `FetcherRateLimit` instance for testing purposes.
    pub fn dummy() -> Self {
        Self {
            limit: 5000,
            cost: 1,
            remaining: 4999,
            reset_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }
}

impl Display for FetcherRateLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RateLimit: calls={}/{} (+{}), reset={}",
            self.limit - self.remaining,
            self.limit,
            self.cost,
            self.reset_at
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod crawler_state {
        use super::*;

        #[tokio::test]
        async fn has_completed_when_target_reached() {
            let state = CrawlerState::default();
            state.set_total_repositories_target(10).await;
            state.increment_total_persisted_repositories(10).await;

            let result = state.has_completed().await.unwrap();

            assert!(result);
        }

        #[tokio::test]
        async fn has_not_completed_and_fails_when_queue_empty_and_requests_pushed_but_not_enough_repositories_persisted()
         {
            let state = CrawlerState::default();
            state.set_total_repositories_target(10).await;
            state.increment_total_persisted_repositories(5).await;
            let request = Request::dummy_search_organization();
            state.push_request(request).await;
            let _ = state.pop_request().await.unwrap();
            assert!(state.pop_request().await.is_none());

            state.has_completed().await.expect_err("Expected an error");
        }

        #[tokio::test]
        async fn has_not_completed_when_queue_not_empty_and_not_enough_repositories_persisted() {
            let state = CrawlerState::default();
            state.set_total_repositories_target(10).await;
            state.increment_total_persisted_repositories(5).await;
            let request = Request::dummy_search_organization();
            state.push_request(request).await;

            let result = state.has_completed().await.unwrap();

            assert!(!result);
        }

        #[tokio::test]
        async fn has_not_completed_when_no_requests_pushed() {
            let state = CrawlerState::default();
            state.set_total_repositories_target(10).await;
            state.increment_total_persisted_repositories(5).await;

            let result = state.has_completed().await.unwrap();

            assert!(!result);
        }

        #[tokio::test]
        async fn push_and_pop_request() {
            let state = CrawlerState::default();
            let request1 = Request::SearchOrganization(crate::SearchOrganizationRequest::new(
                "org-1",
                100,
                Some("after".to_string()),
            ));
            let request2 = Request::SearchOrganization(crate::SearchOrganizationRequest::new(
                "org-2", 100, None,
            ));

            state.push_request(request1.clone()).await;
            state.push_request(request2.clone()).await;
            let popped_request1 = state.pop_request().await;
            let popped_request2 = state.pop_request().await;
            let popped_request3 = state.pop_request().await;

            assert_eq!(popped_request1, Some(request1));
            assert_eq!(popped_request2, Some(request2));
            assert_eq!(popped_request3, None);
        }

        #[tokio::test]
        async fn push_duplicate_request() {
            let state = CrawlerState::default();
            let request = Request::dummy_search_organization();

            state.push_request(request.clone()).await;
            state.push_request(request.clone()).await;
            let popped_request1 = state.pop_request().await;
            let popped_request2 = state.pop_request().await;

            assert_eq!(popped_request1, Some(request));
            assert_eq!(popped_request2, None);
        }

        #[tokio::test]
        async fn set_and_get_total_repositories_target() {
            let state = CrawlerState::default();

            state.set_total_repositories_target(100).await;
            let total_repositories = state.get_total_repositories_target().await;

            assert_eq!(total_repositories, 100);
        }

        #[tokio::test]
        async fn increment_and_get_total_persisted_repositories() {
            let state = CrawlerState::default();

            state.increment_total_persisted_repositories(10).await;
            state.increment_total_persisted_repositories(5).await;
            let total_persisted = state.get_total_persisted_repositories().await;

            assert_eq!(total_persisted, 15);
        }

        #[tokio::test]
        async fn increment_and_get_total_collisions_repositories() {
            let state = CrawlerState::default();

            state.increment_total_collisions_repositories(3).await;
            state.increment_total_collisions_repositories(2).await;
            let total_collisions = state.get_total_collisions_repositories().await;

            assert_eq!(total_collisions, 5);
        }

        #[tokio::test]
        async fn increment_and_get_total_fetcher_calls() {
            let state = CrawlerState::default();

            state.increment_total_fetcher_calls(1).await;
            state.increment_total_fetcher_calls(4).await;
            let total_fetcher_calls = state.get_total_fetcher_calls().await;

            assert_eq!(total_fetcher_calls, 5);
        }

        #[tokio::test]
        async fn update_and_get_current_api_rate_limit() {
            let state = CrawlerState::default();

            let rate_limit = FetcherRateLimit {
                limit: 5000,
                cost: 1,
                remaining: 4999,
                reset_at: "2025-01-01T00:00:00Z".to_string(),
            };

            state
                .update_current_api_rate_limit(rate_limit.clone())
                .await;
            let current_rate_limit = state.get_current_api_rate_limit().await;

            assert_eq!(current_rate_limit, rate_limit);
        }
    }
}
