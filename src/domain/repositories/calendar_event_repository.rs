use crate::common::errors::DomainError;
use crate::domain::entities::calendar_event::CalendarEvent;
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub type CalendarEventRepositoryResult<T> = Result<T, DomainError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarEventPutPrecondition {
    None,
    IfMatch(String),
    IfMatchAny,
    IfNoneMatchAny,
}

#[derive(Debug, Clone)]
pub struct CalendarEventPutResult {
    pub event: CalendarEvent,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarEventDeletePrecondition {
    None,
    IfMatch(String),
    IfMatchAny,
}

pub trait CalendarEventRepository: Send + Sync + 'static {
    async fn create_event(
        &self,
        event: CalendarEvent,
    ) -> CalendarEventRepositoryResult<CalendarEvent>;

    async fn update_event(
        &self,
        event: CalendarEvent,
    ) -> CalendarEventRepositoryResult<CalendarEvent>;

    async fn delete_event(&self, id: &Uuid) -> CalendarEventRepositoryResult<()>;

    async fn delete_event_by_resource_path(
        &self,
        calendar_id: &Uuid,
        resource_path: &str,
        precondition: CalendarEventDeletePrecondition,
    ) -> CalendarEventRepositoryResult<CalendarEvent>;

    async fn find_event_by_id(&self, id: &Uuid) -> CalendarEventRepositoryResult<CalendarEvent>;

    async fn find_event_by_resource_path(
        &self,
        calendar_id: &Uuid,
        resource_path: &str,
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

    async fn put_event_by_resource_path(
        &self,
        event: CalendarEvent,
        precondition: CalendarEventPutPrecondition,
    ) -> CalendarEventRepositoryResult<CalendarEventPutResult>;

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
