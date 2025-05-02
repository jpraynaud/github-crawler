use std::{sync::Arc, time::Duration};

use anyhow::anyhow;
use log::warn;
use tokio::time::sleep;

use crate::{RepositoryCrawler, Request, StdResult};

/// A parallel crawler that uses multiple crawlers to fetch repositories concurrently.
pub struct ParallelCrawler {
    /// The worker crawlers
    crawlers: Vec<Arc<dyn RepositoryCrawler>>,

    /// The delay between starting each crawler
    delay_between_crawlers: Duration,
}

impl ParallelCrawler {
    /// Creates a new `ParallelCrawler` instance with the given crawlers.
    pub fn new(
        crawlers: Vec<Arc<dyn RepositoryCrawler>>,
        delay_between_crawlers: Duration,
    ) -> Self {
        Self {
            crawlers,
            delay_between_crawlers,
        }
    }
}

#[async_trait::async_trait]
impl RepositoryCrawler for ParallelCrawler {
    async fn crawl(&self, requests: Vec<Request>, total_repositories: u32) -> StdResult<()> {
        if requests.is_empty() {
            return Err(anyhow!("No requests provided"));
        }

        let mut handles = Vec::new();
        for crawler in &self.crawlers {
            if !handles.is_empty() {
                sleep(self.delay_between_crawlers).await;
            }
            let requests_clone = requests.clone();
            let crawler_clone = Arc::clone(crawler);
            let handle = tokio::spawn(async move {
                crawler_clone
                    .crawl(requests_clone, total_repositories)
                    .await
            });
            handles.push(handle);
            warn!("Started crawler {}/{}", handles.len(), self.crawlers.len());
        }

        for handle in handles {
            handle.await??;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::{MockRepositoryCrawler, Request};
    use std::sync::Arc;

    #[tokio::test]
    async fn crawl_with_no_requests() {
        let crawler = ParallelCrawler::new(vec![], Duration::from_secs(0));

        crawler
            .crawl(vec![], 10)
            .await
            .expect_err("Crawler should fail with no requests");
    }

    #[tokio::test]
    async fn crawl_with_single_crawler() {
        let mock_crawler = {
            let mut mock_crawler = MockRepositoryCrawler::new();
            mock_crawler
                .expect_crawl()
                .returning(|_, _| Ok(()))
                .times(1);

            mock_crawler
        };
        let crawler = ParallelCrawler::new(vec![Arc::new(mock_crawler)], Duration::from_secs(0));

        crawler
            .crawl(vec![Request::dummy_search_organization()], 10)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn crawl_with_multiple_crawlers() {
        let mock_crawler1 = {
            let mut mock_crawler = MockRepositoryCrawler::new();
            mock_crawler
                .expect_crawl()
                .returning(|_, _| Ok(()))
                .times(1);

            mock_crawler
        };
        let mock_crawler2 = {
            let mut mock_crawler = MockRepositoryCrawler::new();
            mock_crawler
                .expect_crawl()
                .returning(|_, _| Ok(()))
                .times(1);

            mock_crawler
        };
        let crawler = ParallelCrawler::new(
            vec![Arc::new(mock_crawler1), Arc::new(mock_crawler2)],
            Duration::from_secs(0),
        );

        crawler
            .crawl(vec![Request::dummy_search_organization()], 10)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn crawl_with_failing_crawler() {
        let mock_crawler1 = {
            let mut mock_crawler = MockRepositoryCrawler::new();
            mock_crawler
                .expect_crawl()
                .returning(|_, _| Ok(()))
                .times(1);

            mock_crawler
        };
        let mock_crawler2 = {
            let mut mock_crawler = MockRepositoryCrawler::new();
            mock_crawler
                .expect_crawl()
                .returning(|_, _| Err(anyhow!("Crawler failed")))
                .times(1);

            mock_crawler
        };
        let crawler = ParallelCrawler::new(
            vec![Arc::new(mock_crawler1), Arc::new(mock_crawler2)],
            Duration::from_secs(0),
        );

        crawler
            .crawl(vec![Request::dummy_search_organization()], 10)
            .await
            .expect_err("Crawler should fail if one crawler fails");
    }

    #[tokio::test]
    async fn crawl_starts_crawler_with_expected_delay() {
        let now = Utc::now();
        let mock_crawler1 = {
            let mut mock_crawler = MockRepositoryCrawler::new();
            mock_crawler
                .expect_crawl()
                .returning(|_, _| Ok(()))
                .times(1);

            mock_crawler
        };
        let mock_crawler2 = {
            let mut mock_crawler = MockRepositoryCrawler::new();
            mock_crawler
                .expect_crawl()
                .returning(|_, _| Ok(()))
                .times(1);

            mock_crawler
        };
        let crawler = ParallelCrawler::new(
            vec![Arc::new(mock_crawler1), Arc::new(mock_crawler2)],
            Duration::from_secs(1),
        );

        crawler
            .crawl(vec![Request::dummy_search_organization()], 10)
            .await
            .unwrap();
        assert!(now + chrono::Duration::seconds(1) <= Utc::now());
    }
}
