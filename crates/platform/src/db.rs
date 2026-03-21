use anneal_core::{ApplicationError, ApplicationResult};
use sqlx::PgPool;
use std::path::Path;

pub async fn connect_pool(database_url: &str) -> ApplicationResult<PgPool> {
    PgPool::connect(database_url)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
}

pub async fn run_migrations(pool: &PgPool, migrations_dir: &str) -> ApplicationResult<()> {
    let migrator = sqlx::migrate::Migrator::new(Path::new(migrations_dir))
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;

    migrator
        .run(pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
}
