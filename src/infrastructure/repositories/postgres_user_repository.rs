use std::{future::Future, pin::Pin};

use sqlx::{PgPool, Row};

use crate::application::ports::UserRepository;
use crate::domain::entities::user::User;

pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl UserRepository for PostgresUserRepository {
    fn find_by_session_token<'a>(
        &'a self,
        token: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<User>, sqlx::Error>> + Send + 'a>> {
        Box::pin(async move {
            let row = sqlx::query(
                r#"
                SELECT
                    users.id,
                    users.username,
                    users.password_hash,
                    users.created_at,
                    users.updated_at
                FROM users
                INNER JOIN sessions ON sessions.user_id = users.id
                WHERE sessions.token = $1
                  AND sessions.expires_at > NOW()
                "#,
            )
            .bind(token)
            .fetch_optional(&self.pool)
            .await?;

            match row {
                Some(row) => Ok(Some(User {
                    id: row.try_get("id")?,
                    username: row.try_get("username")?,
                    password_hash: row.try_get("password_hash")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                })),
                None => Ok(None),
            }
        })
    }
}
