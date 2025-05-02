use serde::Deserialize;
use std::{fmt::Display, ops::Deref};

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
