use std::sync::Arc;

use clap::Parser;
use log::{debug, info};

use github_crawler::{
    FetcherRetrier, GITHUB_GRAPHQL_ENDPOINT, GraphQlFetcher, PostgresSqlPersister,
    RepositoryCrawler, Request, SearchOrganizationRequest, SequentialCrawler, StdResult,
};

/// Command line arguments for the GitHub crawler
#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Seed queries used to bootstrap crawling
    #[arg(short, long, value_delimiter = ',', default_value = "is:public")]
    seed_queries: Vec<String>,

    /// Total repositories to crawl
    #[arg(short, long, default_value_t = 100000)]
    total_repositories: u32,

    /// Maximum number of repositories fetched per request
    #[arg(short, long, default_value_t = 100)]
    max_repository_fetched_per_request: u16,

    /// PostgreSQL connection string (e.g., postgresql://user:password@localhost:5432/dbname)
    #[arg(short, long)]
    postgres_connection_string: String,
}

#[tokio::main]
async fn main() -> StdResult<()> {
    env_logger::init();
    info!("Starting GitHub crawling");
    let args = Args::parse();
    let total_repositories = args.total_repositories;
    let max_repository_fetched_per_request = args.max_repository_fetched_per_request;
    let requests = prepare_seed_requests(&args.seed_queries, max_repository_fetched_per_request);
    debug!("Seed requests: {requests:?}");

    let crawler = build_sequential_crawler(&args).await?;
    crawler.crawl(requests, total_repositories).await?;
    info!("Crawling completed");

    Ok(())
}

fn prepare_seed_requests(
    seed_queries: &[String],
    max_repository_fetched_per_request: u16,
) -> Vec<Request> {
    seed_queries
        .iter()
        .map(|query| {
            Request::SearchOrganization(SearchOrganizationRequest::new(
                query,
                max_repository_fetched_per_request,
                None,
            ))
        })
        .collect::<Vec<_>>()
}

async fn build_sequential_crawler(args: &Args) -> StdResult<Arc<dyn RepositoryCrawler>> {
    let postgres_connection_string = &args.postgres_connection_string;
    let fetcher = Arc::new(FetcherRetrier::new(
        Arc::new(GraphQlFetcher::try_new(GITHUB_GRAPHQL_ENDPOINT)?),
        3,
    ));
    let persister = Arc::new(PostgresSqlPersister::try_new(postgres_connection_string).await?);

    Ok(Arc::new(SequentialCrawler::new(fetcher, persister)))
}
