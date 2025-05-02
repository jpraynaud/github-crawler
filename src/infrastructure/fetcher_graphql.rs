#![allow(non_snake_case)]

use std::collections::HashMap;

use anyhow::{Context, anyhow};
use gql_client::Client;
use serde::{Deserialize, Serialize};

use crate::{
    FetcherRateLimit, RepositoriesFromOrganizationRequest, Repository, RepositoryFetcher, Request,
    Response, SearchOrganizationRequest, StdResult,
};

/// The GraphQL production endpoint for GitHub.
pub const GITHUB_GRAPHQL_ENDPOINT: &str = "https://api.github.com/graphql";

const SEARCH_QUERY: &str = r#"
query ($query: String!, $first: Int!, $after: String) {
  search(query: $query, type: REPOSITORY, first: $first, after: $after) {
    edges {
      node {
        ... on Repository {
          name
          owner {
            login
          }
          stargazerCount
        }
      }
    }
    pageInfo {
      endCursor
      hasNextPage
    }
  }
  rateLimit {
    limit
    cost
    remaining
    resetAt
  }
}
"#;

#[derive(Deserialize, Debug)]
struct SearchQueryData {
    search: SearchResult,
    rateLimit: RateLimit,
}

#[derive(Deserialize, Debug)]
struct SearchResult {
    edges: Vec<SearchEdge>,
    pageInfo: PageInfo,
}

#[derive(Deserialize, Debug)]
struct SearchEdge {
    node: RepositoryNode,
}

#[derive(Deserialize, Debug)]
struct RepositoryNode {
    name: String,
    owner: Owner,
    stargazerCount: u32,
}

#[derive(Deserialize, Debug)]
struct Owner {
    login: String,
}

#[derive(Deserialize, Debug)]
struct PageInfo {
    endCursor: Option<String>,
    hasNextPage: bool,
}

#[derive(Deserialize, Debug)]
struct RateLimit {
    limit: i32,
    cost: i32,
    remaining: i32,
    resetAt: String,
}

impl From<RateLimit> for FetcherRateLimit {
    fn from(rate_limit: RateLimit) -> Self {
        Self {
            limit: rate_limit.limit,
            cost: rate_limit.cost,
            remaining: rate_limit.remaining,
            reset_at: rate_limit.resetAt,
        }
    }
}

/// A GraphQL query for searching GitHub
#[derive(Debug, Serialize)]
struct GraphQlSearchQuery {
    /// The search query string.
    pub(super) query: String,
    /// The number of repositories to return.
    pub(super) first: u16,
    /// The cursor for pagination.
    pub(super) after: Option<String>,
}

impl From<&SearchOrganizationRequest> for GraphQlSearchQuery {
    fn from(request: &SearchOrganizationRequest) -> Self {
        Self {
            query: request.query.to_owned(),
            first: request.first,
            after: request.after.to_owned(),
        }
    }
}
impl From<&RepositoriesFromOrganizationRequest> for GraphQlSearchQuery {
    fn from(request: &RepositoriesFromOrganizationRequest) -> Self {
        Self {
            query: format!("org:{} stars:>0", request.organization_name),
            first: request.first,
            after: request.after.to_owned(),
        }
    }
}

/// Fetches repository data from a GraphQL API.
pub struct GraphQlFetcher {
    client: Client,
}

impl GraphQlFetcher {
    /// Creates a new `GraphQlFetcher` instance with the given GraphQL client.
    pub fn try_new(endpoint: &str) -> StdResult<Self> {
        let github_api_token = std::env::var("GITHUB_API_TOKEN")
            .with_context(|| "Missing GITHUB_API_TOKEN environment variable")?;
        let bearer_token = format!("Bearer {}", github_api_token);
        let mut headers = HashMap::from([("User-Agent", "gql-client")]);
        headers.insert("Authorization", &bearer_token);
        let client = Client::new_with_headers(endpoint, headers);

        Ok(Self { client })
    }

    async fn fetch_organizations(
        &self,
        request: &SearchOrganizationRequest,
    ) -> StdResult<Option<(Response, Vec<Request>)>> {
        let fetched_data = self
            .client
            .query_with_vars_unwrap::<SearchQueryData, GraphQlSearchQuery>(
                SEARCH_QUERY,
                request.into(),
            )
            .await
            .map_err(|e| anyhow!(e))?;
        if fetched_data.search.edges.is_empty() {
            return Ok(None);
        }

        let mut next_requests = fetched_data
            .search
            .edges
            .into_iter()
            .map(|edge| {
                Request::RepositoriesFromOrganization(RepositoriesFromOrganizationRequest::new(
                    &edge.node.owner.login,
                    request.first,
                    None,
                ))
            })
            .collect::<Vec<_>>();
        if fetched_data.search.pageInfo.hasNextPage {
            next_requests.push(Request::SearchOrganization(SearchOrganizationRequest::new(
                &request.query,
                request.first,
                fetched_data.search.pageInfo.endCursor,
            )));
        }

        Ok(Some((
            Response::new(vec![], fetched_data.rateLimit.into()),
            next_requests,
        )))
    }

    async fn fetch_repositories_from_organization(
        &self,
        request: &RepositoriesFromOrganizationRequest,
    ) -> StdResult<Option<(Response, Vec<Request>)>> {
        let fetched_data = self
            .client
            .query_with_vars_unwrap::<SearchQueryData, GraphQlSearchQuery>(
                SEARCH_QUERY,
                request.into(),
            )
            .await
            .map_err(|e| anyhow!(e))?;
        if fetched_data.search.edges.is_empty() {
            return Ok(None);
        }

        Ok(Some((
            Response::new(
                fetched_data
                    .search
                    .edges
                    .into_iter()
                    .map(|edge| {
                        Repository::new(
                            &edge.node.name,
                            &request.organization_name,
                            edge.node.stargazerCount,
                        )
                    })
                    .collect(),
                fetched_data.rateLimit.into(),
            ),
            if fetched_data.search.pageInfo.hasNextPage {
                vec![Request::RepositoriesFromOrganization(
                    RepositoriesFromOrganizationRequest::new(
                        &request.organization_name,
                        request.first,
                        fetched_data.search.pageInfo.endCursor,
                    ),
                )]
            } else {
                vec![]
            },
        )))
    }
}

#[async_trait::async_trait]
impl RepositoryFetcher for GraphQlFetcher {
    async fn fetch(&self, request: &Request) -> StdResult<Option<(Response, Vec<Request>)>> {
        match request {
            Request::SearchOrganization(request) => self.fetch_organizations(request).await,
            Request::RepositoriesFromOrganization(request) => {
                self.fetch_repositories_from_organization(request).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use httpmock::MockServer;
    use serde_json::json;

    use super::*;

    fn setup_mock_server() -> MockServer {
        let server = MockServer::start();
        unsafe {
            env::set_var("GITHUB_API_TOKEN", "credentials");
        }
        server
    }

    fn mock_json_value() -> serde_json::Value {
        json!({
            "data": {
                "search": {
                    "edges": [
                        {
                            "node": {
                                "name": "repository-1",
                                "owner": {
                                    "login": "org-1"
                                },
                                "stargazerCount": 100
                            }
                        },
                        {
                            "node": {
                                "name": "repository-2",
                                "owner": {
                                    "login": "org-1"
                                },
                                "stargazerCount": 200
                            }
                        }
                    ],
                    "pageInfo": {
                        "endCursor": Some("cursor123".to_string()),
                        "hasNextPage": true
                    }
                },
                "rateLimit": {
                    "limit": 5000,
                    "cost": 1,
                    "remaining": 4999,
                    "resetAt": "2025-01-01T00:00:00Z"
                }
            }
        })
    }

    #[tokio::test]
    async fn test_fetch_organizations() {
        let server = setup_mock_server();
        let mock = server.mock(|when, then| {
            when.method("POST").path("/");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(mock_json_value());
        });
        let fetcher = GraphQlFetcher::try_new(&server.url("/")).unwrap();
        let request = SearchOrganizationRequest::new("stars:>100", 10, None);

        let (response, next_requests) = fetcher
            .fetch_organizations(&request)
            .await
            .unwrap()
            .unwrap();

        mock.assert();
        assert_eq!(Response::new(vec![], FetcherRateLimit::dummy()), response);
        assert_eq!(
            vec![
                Request::RepositoriesFromOrganization(RepositoriesFromOrganizationRequest::new(
                    "org-1", 10, None,
                ),),
                Request::RepositoriesFromOrganization(RepositoriesFromOrganizationRequest::new(
                    "org-1", 10, None,
                ),),
                Request::SearchOrganization(SearchOrganizationRequest::new(
                    "stars:>100",
                    10,
                    Some("cursor123".to_string())
                ),)
            ],
            next_requests
        );
    }

    #[tokio::test]
    async fn test_fetch_repositories_from_organization() {
        let server = setup_mock_server();
        let mock = server.mock(|when, then| {
            when.method("POST").path("/");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(mock_json_value());
        });
        let fetcher = GraphQlFetcher::try_new(&server.url("/")).unwrap();
        let request = RepositoriesFromOrganizationRequest::new("org-1", 10, None);

        let (response, next_requests) = fetcher
            .fetch_repositories_from_organization(&request)
            .await
            .unwrap()
            .unwrap();

        mock.assert();
        assert_eq!(
            Response::new(
                vec![
                    Repository::new("repository-1", "org-1", 100),
                    Repository::new("repository-2", "org-1", 200)
                ],
                FetcherRateLimit::dummy()
            ),
            response
        );
        assert_eq!(
            vec![Request::RepositoriesFromOrganization(
                RepositoriesFromOrganizationRequest::new(
                    "org-1",
                    10,
                    Some("cursor123".to_string()),
                ),
            ),],
            next_requests
        );
    }
}
