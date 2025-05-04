# GitHub crawler

## GitHub action workflow

[![GitHub Crawler](https://github.com/jpraynaud/github-crawler/actions/workflows/crawl.yml/badge.svg)](https://github.com/jpraynaud/github-crawler/actions/workflows/crawl.yml)

This repository contains a GitHub action workflow that crawls the GitHub API to obtain a list of 100,000 repositories and their respective star counts. The data is stored in a PostgreSQL database and can be exported as a CSV file.

The workflow is defined in the `.github/workflows/crawl.yml` file and is triggered manually (it could be triggered on a schedule, but this is not the case here).

## The Crawler CLI

Under the hood, the workflow uses a Rust-based crawler that interacts with the GitHub GraphQL API to fetch repository data. The crawler is designed to respect GitHub's rate limits and includes retry mechanisms for handling errors. The crawler has also builtin concurrency and parallelism features with the use of crawler workers for enhanced efficiency and performances.

### Prerequisites

To begin the setup, ensure the following components and tools are installed:

- A [correctly configured](https://www.rust-lang.org/learn/get-started) Rust toolchain (`1.86+`).
- A [PostgreSQL](https://www.postgresql.org/download/) database server running locally or on a remote server (`16.8+`).

### Build the crawler

To build the crawler, run the following command in the root directory of the repository:

```bash
cargo build --release
```

This will create an executable file named `github-crawler` in the `target/release` directory. The executable is a command-line tool that can be used to crawl the GitHub API and store the data in a PostgreSQL database:

```bash
./target/release/github-crawler --help
```

You should see the following output:

```bash
Command line arguments for the GitHub crawler

Usage: github-crawler [OPTIONS] --postgres-connection-string <POSTGRES_CONNECTION_STRING>

Options:
  -t, --total-repositories <TOTAL_REPOSITORIES>
          Total repositories to crawl [default: 100000]
  -s, --seed-queries <SEED_QUERIES>
          Seed queries used to bootstrap crawling [default: is:public]
  -n, --number-workers <NUMBER_WORKERS>
          Number of workers [default: 1]
  -m, --max-repository-fetched-per-request <MAX_REPOSITORY_FETCHED_PER_REQUEST>
          Maximum number of repositories fetched per request [default: 100]
  -p, --postgres-connection-string <POSTGRES_CONNECTION_STRING>
          PostgreSQL connection string (e.g., postgresql://user:password@localhost:5432/dbname)
  -h, --help
          Print help
  -V, --version
          Print version
```

Optionally, you can run the following command to run the tests:

```bash
cargo test
```

### Run the crawler

To run the crawler, after the build phase, and persist `10,000` repositories and their stars count with `5` parallel workers and log level set to `warn`, use the following command:

```bash
export GITHUB_TOKEN=<YOUR_GITHUB_TOKEN>
export RUST_LOG=warn
export POSTGRES_CONNECTION_STRING=<YOUR_POSTGRES_CONNECTION_STRING>
export TOTAL_REPOSITORIES=10000
export NUMBER_WORKERS=5
export SEED_QUERIES="is:public"

./target/release/github-crawler \
    --total-repositories $TOTAL_REPOSITORIES \
    --seed-queries "$SEED_QUERIES" \
    --number-workers $NUMBER_WORKERS \
    --postgres-connection-string $POSTGRES_CONNECTION_STRING
```

Alternatively, you can build and run the crawler (without the build phase) with the following command:

```bash
cargo run --release -- \
    --total-repositories $TOTAL_REPOSITORIES \
    --seed-queries "$SEED_QUERIES" \
    --number-workers $NUMBER_WORKERS \
    --postgres-connection-string $POSTGRES_CONNECTION_STRING
```

You should see the following output:

```bash
[2025-05-04T19:48:49Z WARN  github_crawler] Starting GitHub crawling
[2025-05-04T19:48:49Z WARN  github_crawler] Seed requests: [SearchOrganization(SearchOrganizationRequest { query: "is:public", first: 100, after: None })]
[2025-05-04T19:48:49Z WARN  github_crawler::infrastructure::crawler_parallel] Repositories: done=0/0, collisions=0, Requests: done=0 in_progress=0 buffered=1, RateLimit: calls=0/0 (+0), reset=
[2025-05-04T19:48:49Z WARN  github_crawler::infrastructure::crawler_parallel] Started crawler 1/5
[2025-05-04T19:48:50Z WARN  github_crawler::infrastructure::crawler_parallel] Started crawler 2/5
[2025-05-04T19:48:51Z WARN  github_crawler::infrastructure::crawler_parallel] Started crawler 3/5
[2025-05-04T19:48:52Z WARN  github_crawler::infrastructure::crawler_parallel] Started crawler 4/5
[2025-05-04T19:48:52Z WARN  github_crawler::infrastructure::crawler_worker] Repositories: done=0/10000, collisions=0, Requests: done=4 in_progress=3 buffered=91, RateLimit: calls=483/5000 (+1), reset=2025-05-04T20:00:54Z
[2025-05-04T19:48:53Z WARN  github_crawler::infrastructure::crawler_parallel] Started crawler 5/5
[2025-05-04T19:48:53Z WARN  github_crawler::infrastructure::crawler_worker] Repositories: done=24/10000, collisions=0, Requests: done=6 in_progress=4 buffered=89, RateLimit: calls=486/5000 (+1), reset=2025-05-04T20:00:54Z
[2025-05-04T19:48:54Z WARN  github_crawler::infrastructure::crawler_worker] Repositories: done=30/10000, collisions=0, Requests: done=7 in_progress=4 buffered=88, RateLimit: calls=489/5000 (+1), reset=2025-05-04T20:00:54Z
[2025-05-04T19:48:54Z WARN  github_crawler::infrastructure::crawler_worker] Repositories: done=50/10000, collisions=0, Requests: done=8 in_progress=4 buffered=87, RateLimit: calls=489/5000 (+1), reset=2025-05-04T20:00:54Z
[2025-05-04T19:48:54Z WARN  github_crawler::infrastructure::crawler_worker] Repositories: done=125/10000, collisions=0, Requests: done=9 in_progress=4 buffered=86, RateLimit: calls=489/5000 (+1), reset=2025-05-04T20:00:54Z
[2025-05-04T19:48:54Z WARN  github_crawler::infrastructure::crawler_worker] Repositories: done=225/10000, collisions=0, Requests: done=10 in_progress=4 buffered=86, RateLimit: calls=489/5000 (+1), reset=2025-05-04T20:00:54Z
[2025-05-04T19:48:54Z WARN  github_crawler::infrastructure::crawler_worker] Repositories: done=230/10000, collisions=0, Requests: done=11 in_progress=4 buffered=85, RateLimit: calls=490/5000 (+1), reset=2025-05-04T20:00:54Z
[2025-05-04T19:48:55Z WARN  github_crawler::infrastructure::crawler_worker] Repositories: done=285/10000, collisions=0, Requests: done=12 in_progress=4 buffered=84, RateLimit: calls=490/5000 (+1), reset=2025-05-04T20:00:54Z

...

[2025-05-04T09:07:59Z WARN  github_crawler] Crawling completed
```

After running the crawler, you can export the crawled data to a CSV file (`repository.csv`) using the following command:

```bash
psql $POSTGRES_CONNECTION_STRING -c "\copy (SELECT repository_name, organization_name, total_stars FROM github.repository LIMIT $TOTAL_REPOSITORIES) to repository.csv with csv header;"
```

### Open the documentation

To open the documentation, run the following command:

```bash
cargo doc --no-deps --open
```
