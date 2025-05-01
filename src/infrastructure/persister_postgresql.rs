use std::ops::Deref;

use log::info;
use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::{Repository, RepositoryPersister, StdResult};

/// A persister that stores repository metadata in a PostgreSQL database.
pub struct PostgresSqlPersister {
    pool: PgPool,
}

impl PostgresSqlPersister {
    /// Creates a new `PostgresSqlPersister` instance.
    pub async fn try_new(connection_string: &str) -> StdResult<Self> {
        Ok(Self {
            pool: PgPoolOptions::new()
                .max_connections(1)
                .connect(connection_string)
                .await?,
        })
    }
}

#[async_trait::async_trait]
impl RepositoryPersister for PostgresSqlPersister {
    async fn persist(&self, data: &[Repository]) -> StdResult<u32> {
        let mut total_inserted = 0;
        for repository in data {
            let repository_name = &*repository.repository_name().deref();
            let organization_name = &*repository.organization_name().deref();
            let repository_stars = *repository.total_stars().deref() as i32;
            let row: (i64,) = sqlx::query_as(
                r#"
WITH upserted AS (
    INSERT INTO github.repository (repository_name, organization_name, total_stars)
    VALUES ($1, $2, $3)
    ON CONFLICT (repository_name, organization_name) DO UPDATE
    SET total_stars = EXCLUDED.total_stars
    WHERE github.repository.repository_name IS DISTINCT FROM EXCLUDED.repository_name
    RETURNING xmax = 0 AS inserted
)
SELECT COUNT(*) AS total_inserted
FROM upserted
WHERE inserted = true;
                "#,
            )
            .bind(repository_name.to_owned())
            .bind(organization_name.to_owned())
            .bind(repository_stars)
            .fetch_one(&self.pool)
            .await?;
            if row.0 == 0 {
                info!("Updated {repository}");
            } else {
                info!("Inserted {repository}");
            }
            total_inserted += row.0 as u32;
        }

        Ok(total_inserted)
    }
}
