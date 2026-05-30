use crate::common::errors::DomainError;
use crate::domain::entities::calendar_event::CalendarEvent;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub type CalendarEventRepositoryResult<T> = Result<T, DomainError>;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum CalendarEventReplaceResult {
    Replaced(CalendarEvent),
    PreconditionFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarEventDeleteCondition {
    None,
    IfMatchAny,
    IfMatch(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarEventDeleteResult {
    Deleted,
    NotFound,
    PreconditionFailed,
}

pub trait CalendarEventRepository: Send + Sync + 'static {
    async fn create_event(
        &self,
        event: CalendarEvent,
    ) -> CalendarEventRepositoryResult<CalendarEvent>;

    async fn create_event_if_resource_absent(
        &self,
        event: CalendarEvent,
    ) -> CalendarEventRepositoryResult<CalendarEvent>;

    async fn update_event(
        &self,
        event: CalendarEvent,
    ) -> CalendarEventRepositoryResult<CalendarEvent>;

    async fn replace_event_by_resource_name(
        &self,
        event: CalendarEvent,
    ) -> CalendarEventRepositoryResult<CalendarEvent>;

    async fn replace_event_by_resource_name_and_etag(
        &self,
        event: CalendarEvent,
        expected_etag: &str,
    ) -> CalendarEventRepositoryResult<CalendarEventReplaceResult>;

    async fn delete_event(&self, id: &Uuid) -> CalendarEventRepositoryResult<()>;

    async fn delete_event_by_resource_name(
        &self,
        calendar_id: &Uuid,
        resource_name: &str,
        condition: CalendarEventDeleteCondition,
    ) -> CalendarEventRepositoryResult<CalendarEventDeleteResult>;

    async fn find_event_by_id(&self, id: &Uuid) -> CalendarEventRepositoryResult<CalendarEvent>;

    async fn find_event_by_resource_name(
        &self,
        calendar_id: &Uuid,
        resource_name: &str,
    ) -> CalendarEventRepositoryResult<Option<CalendarEvent>>;

    async fn list_events_by_calendar(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>>;

    async fn find_events_by_summary(
        &self,
        calendar_id: &Uuid,
        summary: &str,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>>;

    async fn get_events_in_time_range(
        &self,
        calendar_id: &Uuid,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>>;

    async fn find_event_by_ical_uid(
        &self,
        calendar_id: &Uuid,
        ical_uid: &str,
    ) -> CalendarEventRepositoryResult<Option<CalendarEvent>>;

    async fn find_uid_conflict(
        &self,
        calendar_id: &Uuid,
        ical_uid: &str,
        resource_name: &str,
    ) -> CalendarEventRepositoryResult<Option<CalendarEvent>>;

    async fn count_events_in_calendar(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarEventRepositoryResult<i64>;

    async fn delete_all_events_in_calendar(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarEventRepositoryResult<i64>;

    async fn list_events_by_calendar_paginated(
        &self,
        calendar_id: &Uuid,
        limit: i64,
        offset: i64,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>>;

    async fn find_recurring_events_in_range(
        &self,
        calendar_id: &Uuid,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> CalendarEventRepositoryResult<Vec<CalendarEvent>>;
}
