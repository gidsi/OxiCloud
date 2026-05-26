use chrono::Utc;
use sqlx::{PgPool, Row, types::Uuid};
use std::collections::HashMap;
use std::sync::Arc;

use crate::common::errors::DomainError;
use crate::domain::entities::calendar::Calendar;
use crate::domain::repositories::calendar_repository::{
    CalendarRepository, CalendarRepositoryResult,
};

pub struct CalendarPgRepository {
    pool: Arc<PgPool>,
}

fn parse_supported_components(properties: &HashMap<String, String>) -> Vec<String> {
    properties
        .get("{urn:ietf:params:xml:ns:caldav}supported-calendar-component-set")
        .map(|value| {
            value
                .split(',')
                .map(|component| component.trim().to_ascii_uppercase())
                .filter(|component| !component.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|components| !components.is_empty())
        .unwrap_or_else(|| vec!["VEVENT".to_string()])
}

fn calendar_from_row(
    row: &sqlx::postgres::PgRow,
    properties: HashMap<String, String>,
) -> CalendarRepositoryResult<Calendar> {
    Calendar::with_id_and_details(
        row.get("id"),
        row.try_get("name").unwrap_or_else(|_| String::new()),
        row.try_get("slug")
            .unwrap_or_else(|_| row.get::<Uuid, _>("id").to_string()),
        row.get("owner_id"),
        row.get("description"),
        row.get("color"),
        row.try_get("is_public").unwrap_or(false),
        parse_supported_components(&properties),
        row.get("created_at"),
        row.get("updated_at"),
        properties,
    )
    .map_err(|e| DomainError::database_error(format!("Failed to create calendar object: {}", e)))
}

impl CalendarPgRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    async fn load_calendar_properties(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarRepositoryResult<HashMap<String, String>> {
        let rows = sqlx::query(
            r#"
            SELECT name, value
            FROM caldav.calendar_properties
            WHERE calendar_id = $1
            "#,
        )
        .bind(calendar_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get calendar properties: {}", e))
        })?;

        let mut properties = HashMap::new();
        for row in rows {
            properties.insert(row.get("name"), row.get("value"));
        }
        Ok(properties)
    }

    async fn persist_calendar_properties(
        &self,
        calendar: &Calendar,
    ) -> CalendarRepositoryResult<()> {
        for (name, value) in calendar.custom_properties() {
            sqlx::query(
                r#"
                INSERT INTO caldav.calendar_properties (calendar_id, name, value)
                VALUES ($1, $2, $3)
                ON CONFLICT (calendar_id, name) DO UPDATE SET value = EXCLUDED.value
                "#,
            )
            .bind(calendar.id())
            .bind(name)
            .bind(value)
            .execute(&*self.pool)
            .await
            .map_err(|e| {
                DomainError::database_error(format!("Failed to set calendar property: {}", e))
            })?;
        }
        Ok(())
    }
}

impl CalendarRepository for CalendarPgRepository {
    async fn create_calendar(&self, calendar: Calendar) -> CalendarRepositoryResult<Calendar> {
        let row = sqlx::query(
            r#"
            INSERT INTO caldav.calendars (id, slug, name, owner_id, description, color, is_public, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, slug, name, owner_id, description, color, is_public, created_at, updated_at
            "#
        )
        .bind(calendar.id())
        .bind(calendar.slug())
        .bind(calendar.name())
        .bind(calendar.owner_id())
        .bind(calendar.description())
        .bind(calendar.color())
        .bind(calendar.is_public())
        .bind(calendar.created_at())
        .bind(calendar.updated_at())
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(format!("Failed to create calendar: {}", e)))?;

        self.persist_calendar_properties(&calendar).await?;
        let properties = self.load_calendar_properties(&row.get("id")).await?;
        let result = calendar_from_row(&row, properties)?;

        Ok(result)
    }

    async fn update_calendar(&self, calendar: Calendar) -> CalendarRepositoryResult<Calendar> {
        let now = Utc::now();
        let row = sqlx::query(
            r#"
            UPDATE caldav.calendars
            SET slug = $1, name = $2, description = $3, color = $4, is_public = $5, updated_at = $6
            WHERE id = $7
            RETURNING id, slug, name, owner_id, description, color, is_public, created_at, updated_at
            "#,
        )
        .bind(calendar.slug())
        .bind(calendar.name())
        .bind(calendar.description())
        .bind(calendar.color())
        .bind(calendar.is_public())
        .bind(now)
        .bind(calendar.id())
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(format!("Failed to update calendar: {}", e)))?;

        self.persist_calendar_properties(&calendar).await?;
        let properties = self.load_calendar_properties(&row.get("id")).await?;
        let result = calendar_from_row(&row, properties)?;

        Ok(result)
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
        .map_err(|e| DomainError::database_error(format!("Failed to delete calendar: {}", e)))?;

        Ok(())
    }

    async fn find_calendar_by_id(&self, id: &Uuid) -> CalendarRepositoryResult<Calendar> {
        let row = sqlx::query(
            r#"
            SELECT id, slug, name, owner_id, description, color, is_public, created_at, updated_at
            FROM caldav.calendars
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(format!("Failed to get calendar by id: {}", e)))?
        .ok_or_else(|| DomainError::not_found("Calendar", id.to_string()))?;
        let properties = self.load_calendar_properties(&row.get("id")).await?;
        let calendar = calendar_from_row(&row, properties)?;

        Ok(calendar)
    }

    async fn find_calendar_by_slug_and_owner(
        &self,
        slug: &str,
        owner_id: Uuid,
    ) -> CalendarRepositoryResult<Calendar> {
        let row = sqlx::query(
            r#"
            SELECT id, slug, name, owner_id, description, color, is_public, created_at, updated_at
            FROM caldav.calendars
            WHERE slug = $1 AND owner_id = $2
            "#,
        )
        .bind(slug)
        .bind(owner_id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to find calendar by slug and owner: {}", e))
        })?
        .ok_or_else(|| {
            DomainError::not_found("Calendar", format!("{} (owned by {})", slug, owner_id))
        })?;

        let properties = self.load_calendar_properties(&row.get("id")).await?;
        calendar_from_row(&row, properties)
    }

    async fn calendar_exists_by_slug_and_owner(
        &self,
        slug: &str,
        owner_id: Uuid,
    ) -> CalendarRepositoryResult<bool> {
        let row = sqlx::query(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM caldav.calendars WHERE slug = $1 AND owner_id = $2
            ) AS exists
            "#,
        )
        .bind(slug)
        .bind(owner_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to check calendar existence: {}", e))
        })?;

        Ok(row.get("exists"))
    }

    async fn list_calendars_by_owner(
        &self,
        owner_id: Uuid,
    ) -> CalendarRepositoryResult<Vec<Calendar>> {
        let rows = sqlx::query(
            r#"
            SELECT id, slug, name, owner_id, description, color, is_public, created_at, updated_at
            FROM caldav.calendars
            WHERE owner_id = $1
            ORDER BY name
            "#,
        )
        .bind(owner_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get calendars by owner: {}", e))
        })?;

        let mut calendars = Vec::new();
        for row in rows {
            let properties = self.load_calendar_properties(&row.get("id")).await?;
            let calendar = calendar_from_row(&row, properties)?;
            calendars.push(calendar);
        }

        Ok(calendars)
    }

    async fn find_calendar_by_name_and_owner(
        &self,
        name: &str,
        owner_id: Uuid,
    ) -> CalendarRepositoryResult<Calendar> {
        let row = sqlx::query(
            r#"
            SELECT id, slug, name, owner_id, description, color, is_public, created_at, updated_at
            FROM caldav.calendars
            WHERE name = $1 AND owner_id = $2
            "#,
        )
        .bind(name)
        .bind(owner_id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to find calendar by name and owner: {}", e))
        })?
        .ok_or_else(|| {
            DomainError::not_found("Calendar", format!("{} (owned by {})", name, owner_id))
        })?;
        let properties = self.load_calendar_properties(&row.get("id")).await?;
        let calendar = calendar_from_row(&row, properties)?;

        Ok(calendar)
    }

    async fn list_calendars_shared_with_user(
        &self,
        user_id: Uuid,
    ) -> CalendarRepositoryResult<Vec<Calendar>> {
        let rows = sqlx::query(
            r#"
            SELECT c.id, c.name, c.owner_id, c.description, c.color, c.is_public, c.created_at, c.updated_at
            FROM caldav.calendars c
            INNER JOIN caldav.calendar_shares s ON c.id = s.calendar_id
            WHERE s.user_id = $1
            ORDER BY c.name
            "#
        )
        .bind(user_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(format!("Failed to get shared calendars: {}", e)))?;

        let mut calendars = Vec::new();
        for row in rows {
            let properties = self.load_calendar_properties(&row.get("id")).await?;
            let calendar = calendar_from_row(&row, properties)?;
            calendars.push(calendar);
        }

        Ok(calendars)
    }

    async fn list_public_calendars(
        &self,
        limit: i64,
        offset: i64,
    ) -> CalendarRepositoryResult<Vec<Calendar>> {
        let rows = sqlx::query(
            r#"
            SELECT id, slug, name, owner_id, description, color, is_public, created_at, updated_at
            FROM caldav.calendars
            WHERE is_public = true
            ORDER BY name
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get public calendars: {}", e))
        })?;

        let mut calendars = Vec::new();
        for row in rows {
            let properties = self.load_calendar_properties(&row.get("id")).await?;
            let calendar = calendar_from_row(&row, properties)?;
            calendars.push(calendar);
        }

        Ok(calendars)
    }

    async fn user_has_calendar_access(
        &self,
        calendar_id: &Uuid,
        user_id: Uuid,
    ) -> CalendarRepositoryResult<bool> {
        // Check if the user is the owner of the calendar or has a share
        let row = sqlx::query(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM caldav.calendars c
                WHERE c.id = $1 AND (c.owner_id = $2 OR c.is_public = true)
                UNION
                SELECT 1 FROM caldav.calendar_shares s
                WHERE s.calendar_id = $1 AND s.user_id = $2
            ) as has_access
            "#,
        )
        .bind(calendar_id)
        .bind(user_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to check calendar access: {}", e))
        })?;

        Ok(row.get::<bool, _>("has_access"))
    }

    async fn share_calendar(
        &self,
        calendar_id: &Uuid,
        user_id: Uuid,
        access_level: &str,
    ) -> CalendarRepositoryResult<()> {
        // Validate access level
        if !["read", "write", "owner"].contains(&access_level) {
            return Err(DomainError::validation_error(format!(
                "Invalid access level: '{}'. Must be 'read', 'write', or 'owner'",
                access_level
            )));
        }

        sqlx::query(
            r#"
            INSERT INTO caldav.calendar_shares (calendar_id, user_id, access_level)
            VALUES ($1, $2, $3)
            ON CONFLICT (calendar_id, user_id) DO UPDATE SET access_level = $3
            "#,
        )
        .bind(calendar_id)
        .bind(user_id)
        .bind(access_level)
        .execute(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(format!("Failed to share calendar: {}", e)))?;

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
        .map_err(|e| DomainError::database_error(format!("Failed to unshare calendar: {}", e)))?;

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
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get calendar shares: {}", e))
        })?;

        let mut shares = Vec::new();
        for row in rows {
            shares.push((row.get("user_id"), row.get("access_level")));
        }

        Ok(shares)
    }

    async fn get_calendar_property(
        &self,
        calendar_id: &Uuid,
        property_name: &str,
    ) -> CalendarRepositoryResult<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT value
            FROM caldav.calendar_properties
            WHERE calendar_id = $1 AND name = $2
            "#,
        )
        .bind(calendar_id)
        .bind(property_name)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get calendar property: {}", e))
        })?;

        Ok(row.map(|r| r.get("value")))
    }

    async fn set_calendar_property(
        &self,
        calendar_id: &Uuid,
        property_name: &str,
        property_value: &str,
    ) -> CalendarRepositoryResult<()> {
        sqlx::query(
            r#"
            INSERT INTO caldav.calendar_properties (calendar_id, name, value)
            VALUES ($1, $2, $3)
            ON CONFLICT (calendar_id, name) DO UPDATE SET value = $3
            "#,
        )
        .bind(calendar_id)
        .bind(property_name)
        .bind(property_value)
        .execute(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to set calendar property: {}", e))
        })?;

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
            WHERE calendar_id = $1 AND name = $2
            "#,
        )
        .bind(calendar_id)
        .bind(property_name)
        .execute(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to remove calendar property: {}", e))
        })?;

        Ok(())
    }

    async fn get_calendar_properties(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarRepositoryResult<std::collections::HashMap<String, String>> {
        let rows = sqlx::query(
            r#"
            SELECT name, value
            FROM caldav.calendar_properties
            WHERE calendar_id = $1
            "#,
        )
        .bind(calendar_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get calendar properties: {}", e))
        })?;

        let mut properties = std::collections::HashMap::new();
        for row in rows {
            properties.insert(row.get("name"), row.get("value"));
        }

        Ok(properties)
    }
}
