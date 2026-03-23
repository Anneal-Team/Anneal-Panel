use anneal_core::{ApplicationError, ApplicationResult, SecretBox, TokenHasher};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub async fn backfill_protected_data(
    pool: &PgPool,
    secret_box: &SecretBox,
    token_hasher: &TokenHasher,
) -> ApplicationResult<()> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
    sqlx::query("select pg_advisory_xact_lock(hashtext('anneal.backfill_protected_data'))")
        .execute(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
    backfill_users(&mut transaction, secret_box).await?;
    backfill_devices(&mut transaction, secret_box, token_hasher).await?;
    backfill_subscriptions(&mut transaction, secret_box).await?;
    backfill_node_endpoints(&mut transaction, secret_box).await?;
    backfill_config_revisions(&mut transaction, secret_box).await?;
    backfill_deployment_rollouts(&mut transaction, secret_box).await?;
    transaction
        .commit()
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
}

async fn backfill_users(
    transaction: &mut Transaction<'_, Postgres>,
    secret_box: &SecretBox,
) -> ApplicationResult<()> {
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "select id, totp_secret from users where totp_secret is not null",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
    for (id, secret) in rows {
        let protected = secret_box.encrypt(&secret_box.decrypt(&secret)?)?;
        if protected != secret {
            sqlx::query("update users set totp_secret = $2 where id = $1")
                .bind(id)
                .bind(protected)
                .execute(&mut **transaction)
                .await
                .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
    }
    Ok(())
}

async fn backfill_devices(
    transaction: &mut Transaction<'_, Postgres>,
    secret_box: &SecretBox,
    token_hasher: &TokenHasher,
) -> ApplicationResult<()> {
    let rows = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "select id, device_token, device_token_hash from devices",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
    for (id, device_token, device_token_hash) in rows {
        let plaintext = secret_box.decrypt(&device_token)?;
        let protected = secret_box.encrypt(&plaintext)?;
        let hash = token_hasher.hash(&plaintext);
        if protected != device_token || device_token_hash.as_deref() != Some(hash.as_str()) {
            sqlx::query(
                "update devices set device_token = $2, device_token_hash = $3 where id = $1",
            )
            .bind(id)
            .bind(protected)
            .bind(hash)
            .execute(&mut **transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
    }
    Ok(())
}

async fn backfill_subscriptions(
    transaction: &mut Transaction<'_, Postgres>,
    secret_box: &SecretBox,
) -> ApplicationResult<()> {
    let rows = sqlx::query_as::<_, (Uuid, String)>("select id, access_key from subscriptions")
        .fetch_all(&mut **transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
    for (id, access_key) in rows {
        let protected = secret_box.encrypt(&secret_box.decrypt(&access_key)?)?;
        if protected != access_key {
            sqlx::query("update subscriptions set access_key = $2 where id = $1")
                .bind(id)
                .bind(protected)
                .execute(&mut **transaction)
                .await
                .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
    }
    Ok(())
}

async fn backfill_node_endpoints(
    transaction: &mut Transaction<'_, Postgres>,
    secret_box: &SecretBox,
) -> ApplicationResult<()> {
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "select id, reality_private_key from node_endpoints where reality_private_key is not null",
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
    for (id, reality_private_key) in rows {
        let protected = secret_box.encrypt(&secret_box.decrypt(&reality_private_key)?)?;
        if protected != reality_private_key {
            sqlx::query("update node_endpoints set reality_private_key = $2 where id = $1")
                .bind(id)
                .bind(protected)
                .execute(&mut **transaction)
                .await
                .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
    }
    Ok(())
}

async fn backfill_config_revisions(
    transaction: &mut Transaction<'_, Postgres>,
    secret_box: &SecretBox,
) -> ApplicationResult<()> {
    let rows =
        sqlx::query_as::<_, (Uuid, String)>("select id, rendered_config from config_revisions")
            .fetch_all(&mut **transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
    for (id, rendered_config) in rows {
        let protected = secret_box.encrypt(&secret_box.decrypt(&rendered_config)?)?;
        if protected != rendered_config {
            sqlx::query("update config_revisions set rendered_config = $2 where id = $1")
                .bind(id)
                .bind(protected)
                .execute(&mut **transaction)
                .await
                .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
    }
    Ok(())
}

async fn backfill_deployment_rollouts(
    transaction: &mut Transaction<'_, Postgres>,
    secret_box: &SecretBox,
) -> ApplicationResult<()> {
    let rows =
        sqlx::query_as::<_, (Uuid, String)>("select id, rendered_config from deployment_rollouts")
            .fetch_all(&mut **transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
    for (id, rendered_config) in rows {
        let protected = secret_box.encrypt(&secret_box.decrypt(&rendered_config)?)?;
        if protected != rendered_config {
            sqlx::query("update deployment_rollouts set rendered_config = $2 where id = $1")
                .bind(id)
                .bind(protected)
                .execute(&mut **transaction)
                .await
                .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
    }
    Ok(())
}
