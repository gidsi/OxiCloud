use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row, postgres::PgRow, types::Uuid};
use std::sync::Arc;

use crate::common::errors::DomainError;
use crate::domain::entities::calendar_event::CalendarEvent;
use crate::domain::repositories::calendar_event_repository::{
    CalendarEventDeleteCondition, CalendarEventDeleteResult, CalendarEventReplaceResult,
    CalendarEventRepository, CalendarEventRepositoryResult,
};

pub struct CalendarEventPgRepository {
    pool: Arc<PgPool>,
}

impl CalendarEventPgRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    fn select_columns() -> &'static str {
        "id, calendar_id, resource_name, summary, description, location, start_time, end_time, all_day, rrule, created_at, updated_at, ical_uid, ical_data, etag"
    }

    fn row_to_event(row: PgRow) -> CalendarEventRepositoryResult<CalendarEvent> {
        CalendarEvent::with_id_and_metadata(
            row.get("id"),
            row.get("calendar_id"),
            row.get("resource_name"),
            row.get("summary"),
            row.get::<Option<String>, _>("description"),
            row.get::<Option<String>, _>("location"),
            row.get("start_time"),
            row.get("end_time"),
            row.get("all_day"),
            row.get::<Option<String>, _>("rrule"),
            row.get("ical_uid"),
            row.get("ical_data"),
            row.get("etag"),
            row.get("created_at"),
            row.get("updated_at"),
        )
        .map_err(|e| DomainError::database_error(format!("Error creating calendar event: {}", e)))
    }

    fn map_db_error(error: sqlx::Error, resource_name: &str) -> DomainError {
        if let sqlx::Error::Database(db_error) = &error {
            if db_error.is_unique_violation() {
                return DomainError::already_exists("Calendar Event", resource_name.to_string());
            }
        }

        DomainError::database_error(format!(
            "Calendar event database operation failed: {}",
            error
        ))
    }
}

impl CalendarEventRepository for CalendarEventPgRepository {
    async fn create_event(
        &self,
        event: CalendarEvent,
    ) -> CalendarEventRepositoryResult<CalendarEvent> {
        let row = sqlx::query(&format!(
            r#"
            INSERT INTO caldav.calendar_events (
                id, calendar_id, resource_name, summary, description, location, start_time, end_time,
                all_day, rrule, created_at, updated_at, ical_uid, ical_data, etag
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING {}
            "#,
            Self::select_columns()
        ))
        .bind(event.id())
        .bind(event.calendar_id())
        .bind(event.resource_name())
        .bind(event.summary())
        .bind(event.description())
        .bind(event.location())
        .bind(event.start_time())
        .bind(event.end_time())
        .bind(event.all_day())
        .bind(event.rrule())
        .bind(event.created_at())
        .bind(event.updated_at())
        .bind(event.ical_uid())
        .bind(event.ical_data())
        .bind(event.etag())
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| Self::map_db_error(e, event.resource_name()))?;

        Self::row_to_event(row)
    }

    async fn create_event_if_resource_absent(
        &self,
        event: CalendarEvent,
    ) -> CalendarEventRepositoryResult<CalendarEvent> {
        self.create_event(event).await
    }

    async fn update_event(
        &self,
        event: CalendarEvent,
    ) -> CalendarEventRepositoryResult<CalendarEvent> {
        let row = sqlx::query(&format!(
            r#"
            UPDATE caldav.calendar_events
            SET summary = $1,
                description = $2,
                location = $3,
                start_time = $4,
                end_time = $5,
                all_day = $6,
                rrule = $7,
                ical_uid = $8,
                ical_data = $9,
                etag = $10,
                updated_at = $11
            WHERE id = $12
            RETURNING {}
            "#,
            Self::select_columns()
        ))
        .bind(event.summary())
        .bind(event.description())
        .bind(event.location())
        .bind(event.start_time())
        .bind(event.end_time())
        .bind(event.all_day())
        .bind(event.rrule())
        .bind(event.ical_uid())
        .bind(event.ical_data())
        .bind(event.etag())
        .bind(event.updated_at())
        .bind(event.id())
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| Self::map_db_error(e, event.resource_name()))?
        .ok_or_else(|| DomainError::not_found("Calendar Event", event.id().to_string()))?;

        Self::row_to_event(row)
    }

    async fn replace_event_by_resource_name(
        &self,
        event: CalendarEvent,
    ) -> CalendarEventRepositoryResult<CalendarEvent> {
        let row = sqlx::query(&format!(
            r#"
            UPDATE caldav.calendar_events
            SET summary = $1,
                description = $2,
                location = $3,
                start_time = $4,
                end_time = $5,
                all_day = $6,
                rrule = $7,
                ical_uid = $8,
                ical_data = $9,
                etag = $10,
                updated_at = $11
            WHERE calendar_id = $12
              AND resource_name = $13
            RETURNING {}
            "#,
            Self::select_columns()
        ))
        .bind(event.summary())
        .bind(event.description())
        .bind(event.location())
        .bind(event.start_time())
        .bind(event.end_time())
        .bind(event.all_day())
        .bind(event.rrule())
        .bind(event.ical_uid())
        .bind(event.ical_data())
        .bind(event.etag())
        .bind(event.updated_at())
        .bind(event.calendar_id())
        .bind(event.resource_name())
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| Self::map_db_error(e, event.resource_name()))?
        .ok_or_else(|| {
            DomainError::not_found("Calendar Event", event.resource_name().to_string())
        })?;

        Self::row_to_event(row)
    }

    async fn replace_event_by_resource_name_and_etag(
        &self,
        event: CalendarEvent,
        expected_etag: &str,
    ) -> CalendarEventRepositoryResult<CalendarEventReplaceResult> {
        let row = sqlx::query(&format!(
            r#"
            UPDATE caldav.calendar_events
            SET summary = $1,
                description = $2,
                location = $3,
                start_time = $4,
                end_time = $5,
                all_day = $6,
                rrule = $7,
                ical_uid = $8,
                ical_data = $9,
                etag = $10,
                updated_at = $11
            WHERE calendar_id = $12
              AND resource_name = $13
              AND etag = $14
            RETURNING {}
            "#,
            Self::select_columns()
        ))
        .bind(event.summary())
        .bind(event.description())
        .bind(event.location())
        .bind(event.start_time())
        .bind(event.end_time())
        .bind(event.all_day())
        .bind(event.rrule())
        .bind(event.ical_uid())
        .bind(event.ical_data())
        .bind(event.etag())
        .bind(event.updated_at())
        .bind(event.calendar_id())
        .bind(event.resource_name())
        .bind(expected_etag)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| Self::map_db_error(e, event.resource_name()))?;

        match row {
            Some(row) => Ok(CalendarEventReplaceResult::Replaced(Self::row_to_event(
                row,
            )?)),
            None => Ok(CalendarEventReplaceResult::PreconditionFailed),
        }
    }

    async fn delete_event(&self, id: &Uuid) -> CalendarEventRepositoryResult<()> {
        sqlx::query(
            r#"
            DELETE FROM caldav.calendar_events
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to delete calendar event: {}", e))
        })?;

        Ok(())
    }

    async fn delete_event_by_resource_name(
        &self,
        calendar_id: &Uuid,
        resource_name: &str,
        condition: CalendarEventDeleteCondition,
    ) -> CalendarEventRepositoryResult<CalendarEventDeleteResult> {
        let rows_affected = match &condition {
            CalendarEventDeleteCondition::None | CalendarEventDeleteCondition::IfMatchAny => {
                sqlx::query(
                    r#"
                    DELETE FROM caldav.calendar_events
                    WHERE calendar_id = $1
                      AND resource_name = $2
                    "#,
                )
                .bind(calendar_id)
                .bind(resource_name)
                .execute(&*self.pool)
                .await
                .map_err(|e| {
                    DomainError::database_error(format!(
                        "Failed to delete calendar event by resource name: {}",
                        e
                    ))
                })?
                .rows_affected()
            }
            CalendarEventDeleteCondition::IfMatch(etags) => sqlx::query(
                r#"
                    DELETE FROM caldav.calendar_events
                    WHERE calendar_id = $1
                      AND resource_name = $2
                      AND etag = ANY($3::varchar[])
                    "#,
            )
            .bind(calendar_id)
            .bind(resource_name)
            .bind(etags.as_slice())
            .execute(&*self.pool)
            .await
            .map_err(|e| {
                DomainError::database_error(format!(
                    "Failed to delete calendar event by resource name and ETag: {}",
                    e
                ))
            })?
            .rows_affected(),
        };

        if rows_affected > 0 {
            return Ok(CalendarEventDeleteResult::Deleted);
        }

        Ok(match &condition {
            CalendarEventDeleteCondition::None => CalendarEventDeleteResult::NotFound,
            CalendarEventDeleteCondition::IfMatchAny | CalendarEventDeleteCondition::IfMatch(_) => {
                CalendarEventDeleteResult::PreconditionFailed
            }
        })
    }

    async fn find_event_by_id(&self, id: &Uuid) -> CalendarEventRepositoryResult<CalendarEvent> {
        let row = sqlx::query(&format!(
            r#"
            SELECT {}
            FROM caldav.calendar_events
            WHERE id = $1
            "#,
            Self::select_columns()
        ))
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get calendar event by id: {}", e))
        })?
        .ok_or_else(|| DomainError::not_found("Calendar Event", id.to_string()))?;

        Self::row_to_event(row)
    }

    async fn find_event_by_resource_name(
        &self,
        calendar_id: &Uuid,
        resource_name: &str,
    ) -> CalendarEventRepositoryResult<Option<CalendarEvent>> {
        let row = sqlx::query(&format!(
            r#"
            SELECT {}
            FROM caldav.calendar_events
            WHERE calendar_id = $1
              AND resource_name = $2
            "#,
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(resource_name)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!(
                "Failed to get calendar event by resource name: {}",
                e
            ))
        })?;

        row.map(Self::row_to_event).transpose()
    }

    async fn list_events_by_calendar(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT {}
            FROM caldav.calendar_events
            WHERE calendar_id = $1
            ORDER BY start_time
            "#,
            Self::select_columns()
        ))
        .bind(calendar_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get events by calendar: {}", e))
        })?;

        rows.into_iter().map(Self::row_to_event).collect()
    }

    async fn find_events_by_summary(
        &self,
        calendar_id: &Uuid,
        summary: &str,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT {}
            FROM caldav.calendar_events
            WHERE calendar_id = $1
              AND summary ILIKE $2
            ORDER BY start_time
            "#,
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(format!("%{}%", summary))
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to find events by summary: {}", e))
        })?;

        rows.into_iter().map(Self::row_to_event).collect()
    }

    async fn get_events_in_time_range(
        &self,
        calendar_id: &Uuid,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT {}
            FROM caldav.calendar_events
            WHERE calendar_id = $1
              AND (
                  (start_time >= $2 AND start_time < $3) OR
                  (end_time > $2 AND end_time <= $3) OR
                  (start_time <= $2 AND end_time >= $3) OR
                  (rrule IS NOT NULL AND end_time >= $2)
              )
            ORDER BY start_time
            "#,
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(start)
        .bind(end)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get events in time range: {}", e))
        })?;

        rows.into_iter().map(Self::row_to_event).collect()
    }

    async fn find_event_by_ical_uid(
        &self,
        calendar_id: &Uuid,
        ical_uid: &str,
    ) -> CalendarEventRepositoryResult<Option<CalendarEvent>> {
        let row = sqlx::query(&format!(
            r#"
            SELECT {}
            FROM caldav.calendar_events
            WHERE calendar_id = $1
              AND ical_uid = $2
            "#,
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(ical_uid)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to find event by iCal UID: {}", e))
        })?;

        row.map(Self::row_to_event).transpose()
    }

    async fn find_uid_conflict(
        &self,
        calendar_id: &Uuid,
        ical_uid: &str,
        resource_name: &str,
    ) -> CalendarEventRepositoryResult<Option<CalendarEvent>> {
        let row = sqlx::query(&format!(
            r#"
            SELECT {}
            FROM caldav.calendar_events
            WHERE calendar_id = $1
              AND ical_uid = $2
              AND resource_name <> $3
            LIMIT 1
            "#,
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(ical_uid)
        .bind(resource_name)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to find event UID conflict: {}", e))
        })?;

        row.map(Self::row_to_event).transpose()
    }

    async fn count_events_in_calendar(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarEventRepositoryResult<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM caldav.calendar_events
            WHERE calendar_id = $1
            "#,
        )
        .bind(calendar_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to count events in calendar: {}", e))
        })?;

        Ok(row.get("count"))
    }

    async fn delete_all_events_in_calendar(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarEventRepositoryResult<i64> {
        let result = sqlx::query(
            r#"
            DELETE FROM caldav.calendar_events
            WHERE calendar_id = $1
            "#,
        )
        .bind(calendar_id)
        .execute(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to delete events in calendar: {}", e))
        })?;

        Ok(result.rows_affected() as i64)
    }

    async fn list_events_by_calendar_paginated(
        &self,
        calendar_id: &Uuid,
        limit: i64,
        offset: i64,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT {}
            FROM caldav.calendar_events
            WHERE calendar_id = $1
            ORDER BY start_time
            LIMIT $2 OFFSET $3
            "#,
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!(
                "Failed to get paginated events by calendar: {}",
                e
            ))
        })?;

        rows.into_iter().map(Self::row_to_event).collect()
    }

    async fn find_recurring_events_in_range(
        &self,
        calendar_id: &Uuid,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT {}
            FROM caldav.calendar_events
            WHERE calendar_id = $1
              AND rrule IS NOT NULL
              AND end_time >= $2
              AND start_time <= $3
            ORDER BY start_time
            "#,
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(start)
        .bind(end)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to find recurring events in range: {}", e))
        })?;

        rows.into_iter().map(Self::row_to_event).collect()
    }
}
