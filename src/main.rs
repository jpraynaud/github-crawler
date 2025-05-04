use std::{sync::Arc, time::Duration};

use clap::Parser;
use log::warn;

use github_crawler::{
    CrawlerState, FetcherRateLimitEnforcer, FetcherRetrier, GITHUB_GRAPHQL_ENDPOINT,
    GraphQlFetcher, ParallelCrawler, PersisterRetrier, PostgresSqlPersister, RepositoryCrawler,
    Request, SearchOrganizationRequest, StdResult, WorkerCrawler,
};

/// Command line arguments for the GitHub crawler
#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Total repositories to crawl
    #[arg(short, long, default_value_t = 100000)]
    total_repositories: u32,

    /// Seed queries used to bootstrap crawling
    #[arg(short, long, value_delimiter = ',', default_value = "is:public")]
    seed_queries: Vec<String>,

    /// Number of workers
    #[arg(short, long, default_value_t = 1)]
    number_workers: u8,

    /// Maximum number of repositories fetched per request
    #[arg(short, long, default_value_t = 100)]
    max_repository_fetched_per_request: u16,

    /// PostgreSQL connection string (e.g., postgresql://user:password@localhost:5432/dbname)
    #[arg(short, long)]
    postgres_connection_string: String,
}

impl Args {
    async fn build_sequential_crawler(
        &self,
        state: Arc<CrawlerState>,
    ) -> StdResult<Arc<dyn RepositoryCrawler>> {
        // Initialize a fetcher with a rate limit enforcer and a retrier
        const FETCHER_MAX_RETRIES: u32 = 5;
        const FETCHER_RETRY_BASE_DELAY: Duration = Duration::from_secs(10);
        let fetcher = Arc::new(FetcherRetrier::new(
            Arc::new(FetcherRateLimitEnforcer::new(Arc::new(
                GraphQlFetcher::try_new(GITHUB_GRAPHQL_ENDPOINT)?,
            ))),
            FETCHER_MAX_RETRIES,
            FETCHER_RETRY_BASE_DELAY,
        ));

        // Initialize a persister with a retrier
        const PERSISTER_MAX_RETRIES: u32 = 3;
        const PERSISTER_RETRY_BASE_DELAY: Duration = Duration::from_millis(100);
        let persister = Arc::new(PersisterRetrier::new(
            Arc::new(PostgresSqlPersister::try_new(&self.postgres_connection_string).await?),
            PERSISTER_MAX_RETRIES,
            PERSISTER_RETRY_BASE_DELAY,
        ));

        Ok(Arc::new(WorkerCrawler::new(fetcher, persister, state)))
    }

    async fn build_parallel_crawler(
        &self,
        state: Arc<CrawlerState>,
    ) -> StdResult<Arc<dyn RepositoryCrawler>> {
        const DELAY_BETWEEN_CRAWLERS: Duration = Duration::from_secs(1);
        let mut crawlers = Vec::new();
        for _ in 0..self.number_workers {
            crawlers.push(self.build_sequential_crawler(state.clone()).await?);
        }

        Ok(Arc::new(ParallelCrawler::new(
            crawlers,
            DELAY_BETWEEN_CRAWLERS,
        )))
    }

    fn prepare_seed_requests(&self) -> Vec<Request> {
        self.seed_queries
            .iter()
            .map(|query| {
                Request::SearchOrganization(SearchOrganizationRequest::new(
                    query,
                    self.max_repository_fetched_per_request,
                    None,
                ))
            })
            .collect::<Vec<_>>()
    }
}

#[tokio::main]
async fn main() -> StdResult<()> {
    env_logger::init();
    warn!("Starting GitHub crawling");
    let args = Args::parse();
    let total_repositories = args.total_repositories;
    let requests = args.prepare_seed_requests();
    warn!("Seed requests: {requests:?}");

    let state = Arc::new(CrawlerState::default());
    let crawler = args.build_parallel_crawler(state).await?;
    crawler.crawl(requests, total_repositories).await?;
    warn!("Crawling completed");

    Ok(())
}
