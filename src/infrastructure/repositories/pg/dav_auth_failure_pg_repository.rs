use std::sync::Arc;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::application::dtos::dav_auth_failure_dto::{
    CreateDavAuthFailureDto, DavAuthFailureDto,
};
use crate::application::ports::dav_auth_failure_ports::DavAuthFailureStoragePort;
use crate::common::errors::DomainError;
use crate::domain::entities::dav_auth_failure::DavAuthFailure;
use crate::domain::repositories::dav_auth_failure_repository::{
    DavAuthFailureRepository, DavAuthFailureRepositoryResult,
};

pub struct DavAuthFailurePgRepository {
    pool: Arc<PgPool>,
}

impl DavAuthFailurePgRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    fn map_sqlx_error(err: sqlx::Error) -> DomainError {
        match err {
            sqlx::Error::RowNotFound => DomainError::not_found("DavAuthFailure", "id"),
            other => DomainError::database_error(format!(
                "DAV auth failure audit database error: {other}"
            )),
        }
    }

    fn from_row(row: DavAuthFailureRow) -> Result<DavAuthFailure, DomainError> {
        DavAuthFailure::from_data(
            row.id,
            row.occurred_at,
            row.client_ip,
            row.username,
            row.method,
            row.path,
            row.user_agent,
            row.reason,
            row.auth_scheme,
            row.protocol,
        )
    }
}

#[derive(sqlx::FromRow)]
struct DavAuthFailureRow {
    id: Uuid,
    occurred_at: DateTime<Utc>,
    client_ip: String,
    username: String,
    method: String,
    path: String,
    user_agent: String,
    reason: String,
    auth_scheme: String,
    protocol: String,
}

impl DavAuthFailureRepository for DavAuthFailurePgRepository {
    async fn create_failure(
        &self,
        failure: DavAuthFailure,
    ) -> DavAuthFailureRepositoryResult<DavAuthFailure> {
        let row = sqlx::query_as::<_, DavAuthFailureRow>(
            r#"
            INSERT INTO auth.dav_auth_failures (
                id,
                occurred_at,
                client_ip,
                username,
                method,
                path,
                user_agent,
                reason,
                auth_scheme,
                protocol
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING
                id,
                occurred_at,
                client_ip,
                username,
                method,
                path,
                user_agent,
                reason,
                auth_scheme,
                protocol
            "#,
        )
        .bind(failure.id())
        .bind(failure.occurred_at())
        .bind(failure.client_ip())
        .bind(failure.username())
        .bind(failure.method())
        .bind(failure.path())
        .bind(failure.user_agent())
        .bind(failure.reason_code())
        .bind(failure.auth_scheme())
        .bind(failure.protocol())
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Self::map_sqlx_error)?;

        Self::from_row(row)
    }

    async fn get_failure_by_id(&self, id: Uuid) -> DavAuthFailureRepositoryResult<DavAuthFailure> {
        let row = sqlx::query_as::<_, DavAuthFailureRow>(
            r#"
            SELECT
                id,
                occurred_at,
                client_ip,
                username,
                method,
                path,
                user_agent,
                reason,
                auth_scheme,
                protocol
            FROM auth.dav_auth_failures
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Self::map_sqlx_error)?;

        Self::from_row(row)
    }

    async fn count_failures_by_client_ip_since(
        &self,
        client_ip: &str,
        since: DateTime<Utc>,
    ) -> DavAuthFailureRepositoryResult<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM auth.dav_auth_failures
            WHERE client_ip = $1
              AND occurred_at >= $2
            "#,
        )
        .bind(client_ip)
        .bind(since)
        .fetch_one(self.pool.as_ref())
        .await
        .map_err(Self::map_sqlx_error)?;

        Ok(count)
    }

    async fn list_failures_by_client_ip(
        &self,
        client_ip: &str,
        limit: i64,
    ) -> DavAuthFailureRepositoryResult<Vec<DavAuthFailure>> {
        let bounded_limit = limit.clamp(1, 1_000);

        let rows = sqlx::query_as::<_, DavAuthFailureRow>(
            r#"
            SELECT
                id,
                occurred_at,
                client_ip,
                username,
                method,
                path,
                user_agent,
                reason,
                auth_scheme,
                protocol
            FROM auth.dav_auth_failures
            WHERE client_ip = $1
            ORDER BY occurred_at DESC
            LIMIT $2
            "#,
        )
        .bind(client_ip)
        .bind(bounded_limit)
        .fetch_all(self.pool.as_ref())
        .await
        .map_err(Self::map_sqlx_error)?;

        rows.into_iter().map(Self::from_row).collect()
    }
}

impl DavAuthFailureStoragePort for DavAuthFailurePgRepository {
    async fn record_failure(
        &self,
        failure: CreateDavAuthFailureDto,
    ) -> Result<DavAuthFailureDto, DomainError> {
        let entity = DavAuthFailure::try_from(failure)?;
        let saved = self.create_failure(entity).await?;
        Ok(saved.into())
    }

    async fn count_failures_by_client_ip_since(
        &self,
        client_ip: &str,
        since: DateTime<Utc>,
    ) -> Result<i64, DomainError> {
        <Self as DavAuthFailureRepository>::count_failures_by_client_ip_since(
            self, client_ip, since,
        )
        .await
    }

    async fn list_failures_by_client_ip(
        &self,
        client_ip: &str,
        limit: i64,
    ) -> Result<Vec<DavAuthFailureDto>, DomainError> {
        let failures =
            <Self as DavAuthFailureRepository>::list_failures_by_client_ip(self, client_ip, limit)
                .await?;

        Ok(failures.into_iter().map(Into::into).collect())
    }
}
