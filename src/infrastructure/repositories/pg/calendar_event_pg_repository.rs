use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row, postgres::PgRow, types::Uuid};
use std::sync::Arc;

use crate::common::errors::{DomainError, ErrorKind};
use crate::domain::entities::calendar_event::CalendarEvent;
use crate::domain::repositories::calendar_event_repository::{
    CalendarEventDeletePrecondition, CalendarEventPutPrecondition, CalendarEventPutResult,
    CalendarEventRepository, CalendarEventRepositoryResult,
};

pub struct CalendarEventPgRepository {
    pool: Arc<PgPool>,
}

impl CalendarEventPgRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    fn event_from_row(row: &PgRow) -> CalendarEventRepositoryResult<CalendarEvent> {
        CalendarEvent::with_id(
            row.get("id"),
            row.get("calendar_id"),
            row.get("summary"),
            row.get::<Option<String>, _>("description"),
            row.get::<Option<String>, _>("location"),
            row.get("start_time"),
            row.get("end_time"),
            row.get("all_day"),
            row.get::<Option<String>, _>("rrule"),
            row.get("ical_uid"),
            row.get("resource_path"),
            row.get("ical_data"),
            row.get("etag"),
            row.get("created_at"),
            row.get("updated_at"),
        )
        .map_err(|e| DomainError::database_error(format!("Error creating calendar event: {}", e)))
    }

    fn select_columns() -> &'static str {
        "id, calendar_id, summary, description, location, start_time, end_time, all_day, rrule, created_at, updated_at, ical_uid, resource_path, ical_data, etag"
    }

    fn precondition_failed(message: impl Into<String>) -> DomainError {
        DomainError::new(ErrorKind::InvalidInput, "Precondition", message)
    }

    fn uid_conflict(message: impl Into<String>) -> DomainError {
        DomainError::new(ErrorKind::AccessDenied, "CalendarEvent", message)
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
                id, calendar_id, summary, description, location, start_time, end_time,
                all_day, rrule, created_at, updated_at, ical_uid, resource_path, ical_data, etag
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            RETURNING {}
            "#,
            Self::select_columns()
        ))
        .bind(event.id())
        .bind(event.calendar_id())
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
        .bind(event.resource_path())
        .bind(event.ical_data())
        .bind(event.etag())
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.constraint() == Some("idx_calendar_events_calendar_resource_path_unique")
                {
                    return DomainError::already_exists(
                        "CalendarEvent",
                        event.resource_path().to_string(),
                    );
                }
                if db_err.constraint() == Some("idx_calendar_events_calendar_ical_uid_unique") {
                    return DomainError::already_exists(
                        "CalendarEvent",
                        event.ical_uid().to_string(),
                    );
                }
            }
            DomainError::database_error(format!("Failed to create calendar event: {}", e))
        })?;

        Self::event_from_row(&row)
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
                resource_path = $9,
                ical_data = $10,
                etag = $11,
                updated_at = $12
            WHERE id = $13
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
        .bind(event.resource_path())
        .bind(event.ical_data())
        .bind(event.etag())
        .bind(Utc::now())
        .bind(event.id())
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to update calendar event: {}", e))
        })?
        .ok_or_else(|| DomainError::not_found("Calendar Event", event.id().to_string()))?;

        Self::event_from_row(&row)
    }

    async fn delete_event(&self, id: &Uuid) -> CalendarEventRepositoryResult<()> {
        let result = sqlx::query("DELETE FROM caldav.calendar_events WHERE id = $1")
            .bind(id)
            .execute(&*self.pool)
            .await
            .map_err(|e| {
                DomainError::database_error(format!("Failed to delete calendar event: {}", e))
            })?;

        if result.rows_affected() == 0 {
            return Err(DomainError::not_found("Calendar Event", id.to_string()));
        }

        Ok(())
    }

    async fn delete_event_by_resource_path(
        &self,
        calendar_id: &Uuid,
        resource_path: &str,
        precondition: CalendarEventDeletePrecondition,
    ) -> CalendarEventRepositoryResult<CalendarEvent> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            DomainError::database_error(format!("Failed to begin delete transaction: {}", e))
        })?;

        let row = sqlx::query(&format!(
            "SELECT {} FROM caldav.calendar_events WHERE calendar_id = $1 AND resource_path = $2 FOR UPDATE",
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(resource_path)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            DomainError::database_error(format!(
                "Failed to get calendar event by resource path for delete: {}",
                e
            ))
        })?
        .ok_or_else(|| DomainError::not_found("Calendar Event", resource_path.to_string()))?;

        let event = Self::event_from_row(&row)?;

        match precondition {
            CalendarEventDeletePrecondition::None | CalendarEventDeletePrecondition::IfMatchAny => {
            }
            CalendarEventDeletePrecondition::IfMatch(expected_etag) => {
                if event.etag() != expected_etag.trim_matches('"') {
                    return Err(Self::precondition_failed("If-Match precondition failed"));
                }
            }
        }

        let result = sqlx::query(
            "DELETE FROM caldav.calendar_events WHERE calendar_id = $1 AND resource_path = $2",
        )
        .bind(calendar_id)
        .bind(resource_path)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            DomainError::database_error(format!(
                "Failed to delete calendar event by resource path: {}",
                e
            ))
        })?;

        if result.rows_affected() == 0 {
            return Err(DomainError::not_found(
                "Calendar Event",
                resource_path.to_string(),
            ));
        }

        tx.commit().await.map_err(|e| {
            DomainError::database_error(format!("Failed to commit delete transaction: {}", e))
        })?;

        Ok(event)
    }

    async fn find_event_by_id(&self, id: &Uuid) -> CalendarEventRepositoryResult<CalendarEvent> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM caldav.calendar_events WHERE id = $1",
            Self::select_columns()
        ))
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get calendar event by id: {}", e))
        })?
        .ok_or_else(|| DomainError::not_found("Calendar Event", id.to_string()))?;
        Self::event_from_row(&row)
    }

    async fn find_event_by_resource_path(
        &self,
        calendar_id: &Uuid,
        resource_path: &str,
    ) -> CalendarEventRepositoryResult<Option<CalendarEvent>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM caldav.calendar_events WHERE calendar_id = $1 AND resource_path = $2",
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(resource_path)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!(
                "Failed to get calendar event by resource path: {}",
                e
            ))
        })?;
        row.map(|row| Self::event_from_row(&row)).transpose()
    }

    async fn list_events_by_calendar(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>> {
        let rows = sqlx::query(&format!(
            "SELECT {} FROM caldav.calendar_events WHERE calendar_id = $1 ORDER BY start_time",
            Self::select_columns()
        ))
        .bind(calendar_id)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get events by calendar: {}", e))
        })?;
        rows.iter().map(Self::event_from_row).collect()
    }

    async fn find_events_by_summary(
        &self,
        calendar_id: &Uuid,
        summary: &str,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>> {
        let search_pattern = super::like_escape(summary);
        let rows = sqlx::query(&format!("SELECT {} FROM caldav.calendar_events WHERE calendar_id = $1 AND summary ILIKE $2 ORDER BY start_time", Self::select_columns()))
            .bind(calendar_id)
            .bind(&search_pattern)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(format!("Failed to find events by summary: {}", e)))?;
        rows.iter().map(Self::event_from_row).collect()
    }

    async fn get_events_in_time_range(
        &self,
        calendar_id: &Uuid,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>> {
        let rows = sqlx::query(&format!(
            r#"SELECT {} FROM caldav.calendar_events
               WHERE calendar_id = $1
                 AND ((start_time >= $2 AND start_time < $3) OR (end_time > $2 AND end_time <= $3) OR (start_time <= $2 AND end_time >= $3) OR (rrule IS NOT NULL AND end_time >= $2))
               ORDER BY start_time"#,
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(start)
        .bind(end)
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| DomainError::database_error(format!("Failed to get events in time range: {}", e)))?;
        rows.iter().map(Self::event_from_row).collect()
    }

    async fn find_event_by_ical_uid(
        &self,
        calendar_id: &Uuid,
        ical_uid: &str,
    ) -> CalendarEventRepositoryResult<Option<CalendarEvent>> {
        let row = sqlx::query(&format!(
            "SELECT {} FROM caldav.calendar_events WHERE calendar_id = $1 AND ical_uid = $2",
            Self::select_columns()
        ))
        .bind(calendar_id)
        .bind(ical_uid)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to get calendar event by UID: {}", e))
        })?;
        row.map(|row| Self::event_from_row(&row)).transpose()
    }

    async fn put_event_by_resource_path(
        &self,
        event: CalendarEvent,
        precondition: CalendarEventPutPrecondition,
    ) -> CalendarEventRepositoryResult<CalendarEventPutResult> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            DomainError::database_error(format!("Failed to begin event PUT transaction: {}", e))
        })?;

        let existing_row = sqlx::query(&format!("SELECT {} FROM caldav.calendar_events WHERE calendar_id = $1 AND resource_path = $2 FOR UPDATE", Self::select_columns()))
            .bind(event.calendar_id())
            .bind(event.resource_path())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| DomainError::database_error(format!("Failed to lock calendar event: {}", e)))?;

        let existing = existing_row
            .as_ref()
            .map(Self::event_from_row)
            .transpose()?;

        match existing {
            Some(existing_event) => {
                match &precondition {
                    CalendarEventPutPrecondition::None
                    | CalendarEventPutPrecondition::IfMatchAny => {}
                    CalendarEventPutPrecondition::IfNoneMatchAny => {
                        return Err(Self::precondition_failed(
                            "If-None-Match precondition failed",
                        ));
                    }
                    CalendarEventPutPrecondition::IfMatch(expected)
                        if expected == existing_event.etag() => {}
                    CalendarEventPutPrecondition::IfMatch(_) => {
                        return Err(Self::precondition_failed("If-Match precondition failed"));
                    }
                }

                let uid_conflict = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS (SELECT 1 FROM caldav.calendar_events WHERE calendar_id = $1 AND ical_uid = $2 AND id <> $3)"
                )
                .bind(event.calendar_id())
                .bind(event.ical_uid())
                .bind(existing_event.id())
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| DomainError::database_error(format!("Failed to check UID conflict: {}", e)))?;
                if uid_conflict {
                    return Err(Self::uid_conflict(
                        "Calendar object UID conflicts with another resource",
                    ));
                }

                let row = sqlx::query(&format!(
                    r#"UPDATE caldav.calendar_events
                       SET summary = $1, description = $2, location = $3, start_time = $4, end_time = $5,
                           all_day = $6, rrule = $7, ical_uid = $8, ical_data = $9, etag = $10, updated_at = NOW()
                       WHERE id = $11
                       RETURNING {}"#,
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
                .bind(existing_event.id())
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| DomainError::database_error(format!("Failed to update calendar event resource: {}", e)))?;
                tx.commit().await.map_err(|e| {
                    DomainError::database_error(format!(
                        "Failed to commit event PUT transaction: {}",
                        e
                    ))
                })?;
                Ok(CalendarEventPutResult {
                    event: Self::event_from_row(&row)?,
                    created: false,
                })
            }
            None => {
                match &precondition {
                    CalendarEventPutPrecondition::None
                    | CalendarEventPutPrecondition::IfNoneMatchAny => {}
                    CalendarEventPutPrecondition::IfMatch(_)
                    | CalendarEventPutPrecondition::IfMatchAny => {
                        return Err(Self::precondition_failed("If-Match precondition failed"));
                    }
                }

                let uid_conflict = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS (SELECT 1 FROM caldav.calendar_events WHERE calendar_id = $1 AND ical_uid = $2)"
                )
                .bind(event.calendar_id())
                .bind(event.ical_uid())
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| DomainError::database_error(format!("Failed to check UID conflict: {}", e)))?;
                if uid_conflict {
                    return Err(Self::uid_conflict(
                        "Calendar object UID conflicts with another resource",
                    ));
                }

                let row = sqlx::query(&format!(
                    r#"INSERT INTO caldav.calendar_events (
                           id, calendar_id, summary, description, location, start_time, end_time,
                           all_day, rrule, created_at, updated_at, ical_uid, resource_path, ical_data, etag
                       ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,NOW(),NOW(),$10,$11,$12,$13)
                       RETURNING {}"#,
                    Self::select_columns()
                ))
                .bind(event.id())
                .bind(event.calendar_id())
                .bind(event.summary())
                .bind(event.description())
                .bind(event.location())
                .bind(event.start_time())
                .bind(event.end_time())
                .bind(event.all_day())
                .bind(event.rrule())
                .bind(event.ical_uid())
                .bind(event.resource_path())
                .bind(event.ical_data())
                .bind(event.etag())
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| DomainError::database_error(format!("Failed to create calendar event resource: {}", e)))?;
                tx.commit().await.map_err(|e| {
                    DomainError::database_error(format!(
                        "Failed to commit event PUT transaction: {}",
                        e
                    ))
                })?;
                Ok(CalendarEventPutResult {
                    event: Self::event_from_row(&row)?,
                    created: true,
                })
            }
        }
    }

    async fn count_events_in_calendar(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarEventRepositoryResult<i64> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM caldav.calendar_events WHERE calendar_id = $1",
        )
        .bind(calendar_id)
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::database_error(format!("Failed to count events in calendar: {}", e))
        })?;
        Ok(row.get::<i64, _>("count"))
    }

    async fn delete_all_events_in_calendar(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarEventRepositoryResult<i64> {
        let result = sqlx::query("DELETE FROM caldav.calendar_events WHERE calendar_id = $1")
            .bind(calendar_id)
            .execute(&*self.pool)
            .await
            .map_err(|e| {
                DomainError::database_error(format!(
                    "Failed to delete all events in calendar: {}",
                    e
                ))
            })?;
        Ok(result.rows_affected() as i64)
    }

    async fn list_events_by_calendar_paginated(
        &self,
        calendar_id: &Uuid,
        limit: i64,
        offset: i64,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>> {
        let rows = sqlx::query(&format!("SELECT {} FROM caldav.calendar_events WHERE calendar_id = $1 ORDER BY start_time LIMIT $2 OFFSET $3", Self::select_columns()))
            .bind(calendar_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(format!("Failed to get paginated events by calendar: {}", e)))?;
        rows.iter().map(Self::event_from_row).collect()
    }

    async fn find_recurring_events_in_range(
        &self,
        calendar_id: &Uuid,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>> {
        let rows = sqlx::query(&format!("SELECT {} FROM caldav.calendar_events WHERE calendar_id = $1 AND rrule IS NOT NULL AND end_time >= $2 AND start_time <= $3 ORDER BY start_time", Self::select_columns()))
            .bind(calendar_id)
            .bind(start)
            .bind(end)
            .fetch_all(&*self.pool)
            .await
            .map_err(|e| DomainError::database_error(format!("Failed to find recurring events in range: {}", e)))?;
        rows.iter().map(Self::event_from_row).collect()
    }
}
