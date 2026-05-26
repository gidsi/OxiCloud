use sqlx::{PgPool, Row, types::Uuid};
use std::sync::Arc;

use crate::common::errors::{DomainError, ErrorKind};
use crate::domain::entities::calendar::Calendar;
use crate::domain::repositories::calendar_repository::{
    CalendarRepository, CalendarRepositoryResult,
};

pub struct CalendarPgRepository {
    pool: Arc<PgPool>,
}

impl CalendarPgRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    fn row_to_calendar(row: &sqlx::postgres::PgRow) -> CalendarRepositoryResult<Calendar> {
        Calendar::with_dav_metadata(
            row.get("id"),
            row.get("name"),
            row.get("display_name"),
            row.get("owner_id"),
            row.get("description"),
            row.get("color"),
            row.get("is_public"),
            row.get("ctag"),
            row.get("sync_version"),
            row.get("supported_components"),
            row.get("timezone"),
            row.get("calendar_order"),
            row.get("created_at"),
            row.get("updated_at"),
        )
        .map_err(|e| {
            DomainError::database_error(format!("Failed to create calendar object: {}", e))
        })
    }

    fn sqlx_error_to_domain_error(error: sqlx::Error, context: &str) -> DomainError {
        if let sqlx::Error::Database(db_error) = &error {
            if db_error.constraint() == Some("uq_caldav_calendars_owner_name") {
                return DomainError::new(
                    ErrorKind::AlreadyExists,
                    "Calendar",
                    "Calendar collection already exists",
                );
            }
        }

        DomainError::database_error(format!("Failed to {}: {}", context, error))
    }
}

impl CalendarRepository for CalendarPgRepository {
    async fn create_calendar(&self, calendar: Calendar) -> CalendarRepositoryResult<Calendar> {
        let row = sqlx::query(
            r#"
            INSERT INTO caldav.calendars (
                id,
                name,
                display_name,
                owner_id,
                description,
                color,
                is_public,
                ctag,
                sync_version,
                supported_components,
                timezone,
                calendar_order,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            RETURNING
                id,
                name,
                display_name,
                owner_id,
                description,
                color,
                is_public,
                ctag,
                sync_version,
                supported_components,
                timezone,
                calendar_order,
                created_at,
                updated_at
            "#,
        )
        .bind(calendar.id())
        .bind(calendar.name())
        .bind(calendar.display_name())
        .bind(calendar.owner_id())
        .bind(calendar.description())
        .bind(calendar.color())
        .bind(calendar.is_public())
        .bind(calendar.ctag())
        .bind(calendar.sync_version())
        .bind(calendar.supported_components())
        .bind(calendar.timezone())
        .bind(calendar.calendar_order())
        .bind(calendar.created_at())
        .bind(calendar.updated_at())
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "create calendar"))?;

        Self::row_to_calendar(&row)
    }

    async fn update_calendar(&self, calendar: Calendar) -> CalendarRepositoryResult<Calendar> {
        let row = sqlx::query(
            r#"
            UPDATE caldav.calendars
            SET
                name = $1,
                display_name = $2,
                description = $3,
                color = $4,
                is_public = $5,
                ctag = $6,
                sync_version = $7,
                supported_components = $8,
                timezone = $9,
                calendar_order = $10,
                updated_at = $11
            WHERE id = $12
            RETURNING
                id,
                name,
                display_name,
                owner_id,
                description,
                color,
                is_public,
                ctag,
                sync_version,
                supported_components,
                timezone,
                calendar_order,
                created_at,
                updated_at
            "#,
        )
        .bind(calendar.name())
        .bind(calendar.display_name())
        .bind(calendar.description())
        .bind(calendar.color())
        .bind(calendar.is_public())
        .bind(calendar.ctag())
        .bind(calendar.sync_version())
        .bind(calendar.supported_components())
        .bind(calendar.timezone())
        .bind(calendar.calendar_order())
        .bind(calendar.updated_at())
        .bind(calendar.id())
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "update calendar"))?;

        Self::row_to_calendar(&row)
    }

    async fn delete_calendar(&self, id: &Uuid) -> CalendarRepositoryResult<()> {
        sqlx::query(
            r#"
            DELETE FROM caldav.calendars
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "delete calendar"))?;

        Ok(())
    }

    async fn find_calendar_by_id(&self, id: &Uuid) -> CalendarRepositoryResult<Calendar> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                name,
                display_name,
                owner_id,
                description,
                color,
                is_public,
                ctag,
                sync_version,
                supported_components,
                timezone,
                calendar_order,
                created_at,
                updated_at
            FROM caldav.calendars
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "get calendar by id"))?
        .ok_or_else(|| DomainError::not_found("Calendar", id.to_string()))?;

        Self::row_to_calendar(&row)
    }

    async fn list_calendars_by_owner(
        &self,
        owner_id: Uuid,
    ) -> CalendarRepositoryResult<Vec<Calendar>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                name,
                display_name,
                owner_id,
                description,
                color,
                is_public,
                ctag,
                sync_version,
                supported_components,
                timezone,
                calendar_order,
                created_at,
                updated_at
            FROM caldav.calendars
            WHERE owner_id = $1
            ORDER BY calendar_order ASC, display_name ASC
            "#,
        )
        .bind(owner_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "list calendars by owner"))?;

        rows.iter().map(Self::row_to_calendar).collect()
    }

    async fn find_calendar_by_name_and_owner(
        &self,
        name: &str,
        owner_id: Uuid,
    ) -> CalendarRepositoryResult<Calendar> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                name,
                display_name,
                owner_id,
                description,
                color,
                is_public,
                ctag,
                sync_version,
                supported_components,
                timezone,
                calendar_order,
                created_at,
                updated_at
            FROM caldav.calendars
            WHERE name = $1 AND owner_id = $2
            "#,
        )
        .bind(name)
        .bind(owner_id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "find calendar by name and owner"))?
        .ok_or_else(|| {
            DomainError::not_found("Calendar", format!("{} (owned by {})", name, owner_id))
        })?;

        Self::row_to_calendar(&row)
    }

    async fn list_calendars_shared_with_user(
        &self,
        user_id: Uuid,
    ) -> CalendarRepositoryResult<Vec<Calendar>> {
        let rows = sqlx::query(
            r#"
            SELECT
                c.id,
                c.name,
                c.display_name,
                c.owner_id,
                c.description,
                c.color,
                c.is_public,
                c.ctag,
                c.sync_version,
                c.supported_components,
                c.timezone,
                c.calendar_order,
                c.created_at,
                c.updated_at
            FROM caldav.calendars c
            INNER JOIN caldav.calendar_shares s ON c.id = s.calendar_id
            WHERE s.user_id = $1
            ORDER BY c.calendar_order ASC, c.display_name ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "list shared calendars"))?;

        rows.iter().map(Self::row_to_calendar).collect()
    }

    async fn list_public_calendars(
        &self,
        limit: i64,
        offset: i64,
    ) -> CalendarRepositoryResult<Vec<Calendar>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                name,
                display_name,
                owner_id,
                description,
                color,
                is_public,
                ctag,
                sync_version,
                supported_components,
                timezone,
                calendar_order,
                created_at,
                updated_at
            FROM caldav.calendars
            WHERE is_public = true
            ORDER BY calendar_order ASC, display_name ASC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "list public calendars"))?;

        rows.iter().map(Self::row_to_calendar).collect()
    }

    async fn user_has_calendar_access(
        &self,
        calendar_id: &Uuid,
        user_id: Uuid,
    ) -> CalendarRepositoryResult<bool> {
        let row = sqlx::query(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM caldav.calendars c
                WHERE c.id = $1 AND (c.owner_id = $2 OR c.is_public = true)

                UNION

                SELECT 1
                FROM caldav.calendar_shares s
                WHERE s.calendar_id = $1 AND s.user_id = $2
            ) AS has_access
            "#,
        )
        .bind(calendar_id)
        .bind(user_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "check calendar access"))?;

        Ok(row.get::<bool, _>("has_access"))
    }

    async fn share_calendar(
        &self,
        calendar_id: &Uuid,
        user_id: Uuid,
        access_level: &str,
    ) -> CalendarRepositoryResult<()> {
        if !["read", "write", "owner"].contains(&access_level) {
            return Err(DomainError::validation_error(format!(
                "Invalid access level: '{}'. Must be 'read', 'write', or 'owner'",
                access_level
            )));
        }

        sqlx::query(
            r#"
            INSERT INTO caldav.calendar_shares (
                id,
                calendar_id,
                user_id,
                access_level
            )
            VALUES (gen_random_uuid(), $1, $2, $3)
            ON CONFLICT (calendar_id, user_id)
            DO UPDATE SET access_level = $3
            "#,
        )
        .bind(calendar_id)
        .bind(user_id)
        .bind(access_level)
        .execute(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "share calendar"))?;

        Ok(())
    }

    async fn remove_calendar_sharing(
        &self,
        calendar_id: &Uuid,
        user_id: Uuid,
    ) -> CalendarRepositoryResult<()> {
        sqlx::query(
            r#"
            DELETE FROM caldav.calendar_shares
            WHERE calendar_id = $1 AND user_id = $2
            "#,
        )
        .bind(calendar_id)
        .bind(user_id)
        .execute(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "remove calendar sharing"))?;

        Ok(())
    }

    async fn get_calendar_shares(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarRepositoryResult<Vec<(String, String)>> {
        let rows = sqlx::query(
            r#"
            SELECT user_id, access_level
            FROM caldav.calendar_shares
            WHERE calendar_id = $1
            ORDER BY user_id
            "#,
        )
        .bind(calendar_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "get calendar shares"))?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let user_id: Uuid = row.get("user_id");
                let access_level: String = row.get("access_level");
                (user_id.to_string(), access_level)
            })
            .collect())
    }

    async fn get_calendar_property(
        &self,
        calendar_id: &Uuid,
        property_name: &str,
    ) -> CalendarRepositoryResult<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT property_value
            FROM caldav.calendar_properties
            WHERE calendar_id = $1 AND property_name = $2
            "#,
        )
        .bind(calendar_id)
        .bind(property_name)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "get calendar property"))?;

        Ok(row.map(|r| r.get("property_value")))
    }

    async fn set_calendar_property(
        &self,
        calendar_id: &Uuid,
        property_name: &str,
        property_value: &str,
    ) -> CalendarRepositoryResult<()> {
        sqlx::query(
            r#"
            INSERT INTO caldav.calendar_properties (
                id,
                calendar_id,
                property_name,
                property_value,
                namespace
            )
            VALUES (gen_random_uuid(), $1, $2, $3, NULL)
            ON CONFLICT (calendar_id, property_name)
            DO UPDATE SET property_value = $3
            "#,
        )
        .bind(calendar_id)
        .bind(property_name)
        .bind(property_value)
        .execute(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "set calendar property"))?;

        Ok(())
    }

    async fn remove_calendar_property(
        &self,
        calendar_id: &Uuid,
        property_name: &str,
    ) -> CalendarRepositoryResult<()> {
        sqlx::query(
            r#"
            DELETE FROM caldav.calendar_properties
            WHERE calendar_id = $1 AND property_name = $2
            "#,
        )
        .bind(calendar_id)
        .bind(property_name)
        .execute(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "remove calendar property"))?;

        Ok(())
    }

    async fn get_calendar_properties(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarRepositoryResult<std::collections::HashMap<String, String>> {
        let rows = sqlx::query(
            r#"
            SELECT property_name, property_value
            FROM caldav.calendar_properties
            WHERE calendar_id = $1
            "#,
        )
        .bind(calendar_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| Self::sqlx_error_to_domain_error(e, "get calendar properties"))?;

        let mut properties = std::collections::HashMap::new();
        for row in rows {
            properties.insert(row.get("property_name"), row.get("property_value"));
        }

        Ok(properties)
    }
}
