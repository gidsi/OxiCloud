use std::sync::Arc;

use sqlx::PgPool;
use uuid::Uuid;

use crate::application::dtos::dav_principal_dto::DavPrincipalDto;
use crate::application::ports::dav_principal_ports::DavPrincipalStoragePort;
use crate::common::errors::DomainError;
use crate::domain::entities::dav_principal::DavPrincipal;
use crate::domain::repositories::dav_principal_repository::{
    DavPrincipalRepository, DavPrincipalRepositoryResult,
};

pub struct DavPrincipalPgRepository {
    pool: Arc<PgPool>,
}

impl DavPrincipalPgRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    fn map_sqlx_error(err: sqlx::Error) -> DomainError {
        match err {
            sqlx::Error::RowNotFound => DomainError::not_found("DavPrincipal", "user"),
            other => DomainError::database_error(format!("DAV principal database error: {other}")),
        }
    }

    fn from_row(row: DavPrincipalRow) -> Result<DavPrincipal, DomainError> {
        DavPrincipal::from_data(
            row.user_id,
            row.username,
            row.principal_path,
            row.calendar_home_set_path,
            row.addressbook_home_set_path,
            row.created_at,
            row.updated_at,
        )
    }
}

#[derive(sqlx::FromRow)]
struct DavPrincipalRow {
    user_id: Uuid,
    username: String,
    principal_path: String,
    calendar_home_set_path: String,
    addressbook_home_set_path: String,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl DavPrincipalRepository for DavPrincipalPgRepository {
    async fn upsert_principal(
        &self,
        principal: DavPrincipal,
    ) -> DavPrincipalRepositoryResult<DavPrincipal> {
        let row = sqlx::query_as::<_, DavPrincipalRow>(
            r#"
            INSERT INTO dav.principals (
                user_id,
                username,
                principal_path,
                calendar_home_set_path,
                addressbook_home_set_path,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (user_id) DO UPDATE
            SET
                username = EXCLUDED.username,
                principal_path = EXCLUDED.principal_path,
                calendar_home_set_path = EXCLUDED.calendar_home_set_path,
                addressbook_home_set_path = EXCLUDED.addressbook_home_set_path,
                updated_at = CURRENT_TIMESTAMP
            RETURNING
                user_id,
                username,
                principal_path,
                calendar_home_set_path,
                addressbook_home_set_path,
                created_at,
                updated_at
            "#,
        )
        .bind(principal.user_id())
        .bind(principal.username())
        .bind(principal.principal_path())
        .bind(principal.calendar_home_set_path())
        .bind(principal.addressbook_home_set_path())
        .bind(principal.created_at())
        .bind(principal.updated_at())
        .fetch_one(&*self.pool)
        .await
        .map_err(Self::map_sqlx_error)?;

        Self::from_row(row)
    }

    async fn get_principal_by_user_id(
        &self,
        user_id: Uuid,
    ) -> DavPrincipalRepositoryResult<DavPrincipal> {
        let row = sqlx::query_as::<_, DavPrincipalRow>(
            r#"
            SELECT
                user_id,
                username,
                principal_path,
                calendar_home_set_path,
                addressbook_home_set_path,
                created_at,
                updated_at
            FROM dav.principals
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(Self::map_sqlx_error)?;

        Self::from_row(row)
    }

    async fn get_principal_by_principal_path(
        &self,
        principal_path: &str,
    ) -> DavPrincipalRepositoryResult<DavPrincipal> {
        let row = sqlx::query_as::<_, DavPrincipalRow>(
            r#"
            SELECT
                user_id,
                username,
                principal_path,
                calendar_home_set_path,
                addressbook_home_set_path,
                created_at,
                updated_at
            FROM dav.principals
            WHERE principal_path = $1
            "#,
        )
        .bind(principal_path)
        .fetch_one(&*self.pool)
        .await
        .map_err(Self::map_sqlx_error)?;

        Self::from_row(row)
    }

    async fn delete_principal(&self, user_id: Uuid) -> DavPrincipalRepositoryResult<()> {
        sqlx::query("DELETE FROM dav.principals WHERE user_id = $1")
            .bind(user_id)
            .execute(&*self.pool)
            .await
            .map_err(Self::map_sqlx_error)?;

        Ok(())
    }
}

impl DavPrincipalStoragePort for DavPrincipalPgRepository {
    async fn get_principal_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<DavPrincipalDto, DomainError> {
        Ok(
            <Self as DavPrincipalRepository>::get_principal_by_user_id(self, user_id)
                .await?
                .into(),
        )
    }

    async fn get_principal_by_principal_path(
        &self,
        principal_path: &str,
    ) -> Result<DavPrincipalDto, DomainError> {
        Ok(
            <Self as DavPrincipalRepository>::get_principal_by_principal_path(self, principal_path)
                .await?
                .into(),
        )
    }

    async fn upsert_principal_for_user(
        &self,
        user_id: Uuid,
        username: &str,
    ) -> Result<DavPrincipalDto, DomainError> {
        let principal = DavPrincipal::new(user_id, username.to_string())?;

        Ok(
            <Self as DavPrincipalRepository>::upsert_principal(self, principal)
                .await?
                .into(),
        )
    }
}
