use std::{cmp::Ordering, fmt::Display};

use serde::Serialize;

/// A request to the GitHub API
#[derive(Debug, Serialize, PartialEq, Eq, Clone, Hash)]
pub enum Request {
    /// A request to fetch organizations from the GitHub API.
    SearchOrganization(SearchOrganizationRequest),

    /// A request to fetch repository metadata from the GitHub API for a specific organization.
    RepositoriesFromOrganization(RepositoriesFromOrganizationRequest),
}

impl Request {
    fn get_first(&self) -> u16 {
        match self {
            Request::SearchOrganization(request) => request.first,
            Request::RepositoriesFromOrganization(request) => request.first,
        }
    }

    fn get_after(&self) -> Option<String> {
        match self {
            Request::SearchOrganization(request) => request.after.clone(),
            Request::RepositoriesFromOrganization(request) => request.after.clone(),
        }
    }

    fn get_variant_weight(&self) -> u16 {
        match self {
            Request::SearchOrganization(_) => 0,
            Request::RepositoriesFromOrganization(_) => 1,
        }
    }

    /// Creates a dummy `SearchOrganization` request for testing purposes.
    #[cfg(test)]
    pub(crate) fn dummy_search_organization() -> Self {
        Self::SearchOrganization(SearchOrganizationRequest::dummy())
    }
}

impl PartialOrd for Request {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Request {
    fn cmp(&self, other: &Self) -> Ordering {
        self.get_after()
            .cmp(&other.get_after())
            .then_with(|| self.get_variant_weight().cmp(&other.get_variant_weight()))
            .then_with(|| self.get_first().cmp(&other.get_first()))
            .then_with(|| match self {
                Request::SearchOrganization(request) => {
                    request.query.cmp(&other.get_after().unwrap_or_default())
                }
                Request::RepositoriesFromOrganization(request) => request
                    .organization_name
                    .cmp(&other.get_after().unwrap_or_default()),
            })
    }
}

impl Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Request::SearchOrganization(request) => write!(f, "{}", request),
            Request::RepositoriesFromOrganization(request) => write!(f, "{}", request),
        }
    }
}

/// A search request being made to the GitHub API
#[derive(Debug, Serialize, PartialEq, Eq, Clone, Hash)]
pub struct SearchOrganizationRequest {
    /// The text query.
    pub(crate) query: String,

    /// The number of repositories to return.
    pub(crate) first: u16,

    /// The cursor for pagination.
    pub(crate) after: Option<String>,
}

impl SearchOrganizationRequest {
    /// Creates a new `SearchOrganizationRequest` with the given query, first, and after values.
    pub fn new(query: &str, first: u16, after: Option<String>) -> Self {
        Self {
            query: query.to_string(),
            first,
            after,
        }
    }

    /// Creates a dummy `SearchOrganizationRequest` for testing purposes.
    #[cfg(test)]
    pub(crate) fn dummy() -> Self {
        Self {
            query: "dummy".to_string(),
            first: 10,
            after: None,
        }
    }
}

impl Display for SearchOrganizationRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SearchOrganizationRequest: query={}, first={}, after={:?}",
            self.query, self.first, self.after
        )
    }
}

/// A repository from organization request being made to the GitHub API
#[derive(Debug, Serialize, PartialEq, Eq, Clone, Hash)]
pub struct RepositoriesFromOrganizationRequest {
    /// The organization name.
    pub(crate) organization_name: String,

    /// The number of repositories to return.
    pub(crate) first: u16,

    /// The cursor for pagination.
    pub(crate) after: Option<String>,
}

impl RepositoriesFromOrganizationRequest {
    /// Creates a new `Request` with the given organization name, first, and after values.
    pub fn new(organization_name: &str, first: u16, after: Option<String>) -> Self {
        Self {
            organization_name: organization_name.to_string(),
            first,
            after,
        }
    }
}

impl Display for RepositoriesFromOrganizationRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RepositoriesFromOrganizationRequest: organization_name={}, first={}, after={:?}",
            self.organization_name, self.first, self.after
        )
    }
}
