use crate::application::dtos::calendar_dto::{
    CalendarAccessDto, CalendarDto, CalendarEventDto, CalendarObjectDeleteResultDto,
    CalendarObjectPutResultDto, CreateCalendarDto, CreateEventDto, CreateEventICalDto,
    DeleteCalendarObjectDto, PutCalendarObjectDto, UpdateCalendarDto, UpdateEventDto,
};
use crate::common::errors::DomainError;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Port for external calendar storage mechanisms
pub trait CalendarStoragePort: Send + Sync + 'static {
    async fn create_calendar(
        &self,
        calendar: CreateCalendarDto,
        owner_id: Uuid,
    ) -> Result<CalendarDto, DomainError>;

    async fn update_calendar(
        &self,
        calendar_id: &str,
        update: UpdateCalendarDto,
    ) -> Result<CalendarDto, DomainError>;

    async fn delete_calendar(&self, calendar_id: &str) -> Result<(), DomainError>;

    async fn get_calendar(&self, calendar_id: &str) -> Result<CalendarDto, DomainError>;

    async fn find_calendar_by_slug_for_owner(
        &self,
        slug: &str,
        owner_id: Uuid,
    ) -> Result<CalendarDto, DomainError>;

    async fn list_calendars_by_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<CalendarDto>, DomainError>;

    async fn list_calendars_shared_with_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<CalendarDto>, DomainError>;

    async fn list_public_calendars(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CalendarDto>, DomainError>;

    async fn get_calendar_access(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<CalendarAccessDto, DomainError>;

    async fn check_calendar_access(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<bool, DomainError>;

    async fn check_calendar_write_access(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<bool, DomainError>;

    async fn share_calendar(
        &self,
        calendar_id: &str,
        user_id: Uuid,
        access_level: &str,
    ) -> Result<(), DomainError>;

    async fn remove_calendar_sharing(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<(), DomainError>;

    async fn get_calendar_shares(
        &self,
        calendar_id: &str,
    ) -> Result<Vec<(String, String)>, DomainError>;

    async fn set_calendar_property(
        &self,
        calendar_id: &str,
        property_name: &str,
        property_value: &str,
    ) -> Result<(), DomainError>;

    async fn get_calendar_property(
        &self,
        calendar_id: &str,
        property_name: &str,
    ) -> Result<Option<String>, DomainError>;

    async fn get_calendar_properties(
        &self,
        calendar_id: &str,
    ) -> Result<std::collections::HashMap<String, String>, DomainError>;

    async fn create_event(&self, event: CreateEventDto) -> Result<CalendarEventDto, DomainError>;

    async fn create_event_from_ical(
        &self,
        event: CreateEventICalDto,
    ) -> Result<CalendarEventDto, DomainError>;

    async fn put_calendar_object(
        &self,
        put: PutCalendarObjectDto,
    ) -> Result<CalendarObjectPutResultDto, DomainError>;

    async fn delete_calendar_object(
        &self,
        dto: DeleteCalendarObjectDto,
    ) -> Result<CalendarObjectDeleteResultDto, DomainError>;

    async fn update_event(
        &self,
        event_id: &str,
        update: UpdateEventDto,
    ) -> Result<CalendarEventDto, DomainError>;

    async fn delete_event(&self, event_id: &str) -> Result<(), DomainError>;

    async fn get_event(&self, event_id: &str) -> Result<CalendarEventDto, DomainError>;

    async fn get_event_by_resource_name(
        &self,
        calendar_id: &str,
        resource_name: &str,
    ) -> Result<Option<CalendarEventDto>, DomainError>;

    async fn list_events_by_calendar(
        &self,
        calendar_id: &str,
    ) -> Result<Vec<CalendarEventDto>, DomainError>;

    async fn list_events_by_calendar_paginated(
        &self,
        calendar_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CalendarEventDto>, DomainError>;

    async fn get_events_in_time_range(
        &self,
        calendar_id: &str,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> Result<Vec<CalendarEventDto>, DomainError>;
}

/// Port for calendar use cases.
pub trait CalendarUseCase: Send + Sync + 'static {
    async fn create_calendar(
        &self,
        calendar: CreateCalendarDto,
        user_id: Uuid,
    ) -> Result<CalendarDto, DomainError>;

    async fn update_calendar(
        &self,
        calendar_id: &str,
        update: UpdateCalendarDto,
        user_id: Uuid,
    ) -> Result<CalendarDto, DomainError>;

    async fn delete_calendar(&self, calendar_id: &str, user_id: Uuid) -> Result<(), DomainError>;

    async fn get_calendar(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<CalendarDto, DomainError>;

    async fn get_calendar_access(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<CalendarAccessDto, DomainError>;

    async fn ensure_calendar_read_access(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<(), DomainError>;

    async fn ensure_calendar_write_access(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<(), DomainError>;

    async fn find_calendar_by_slug_for_owner(
        &self,
        slug: &str,
        owner_id: Uuid,
    ) -> Result<CalendarDto, DomainError>;

    async fn list_my_calendars(&self, user_id: Uuid) -> Result<Vec<CalendarDto>, DomainError>;

    async fn list_shared_calendars(&self, user_id: Uuid) -> Result<Vec<CalendarDto>, DomainError>;

    async fn list_public_calendars(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<CalendarDto>, DomainError>;

    async fn share_calendar(
        &self,
        calendar_id: &str,
        target_user_id: Uuid,
        access_level: &str,
        caller_user_id: Uuid,
    ) -> Result<(), DomainError>;

    async fn remove_calendar_sharing(
        &self,
        calendar_id: &str,
        target_user_id: Uuid,
        caller_user_id: Uuid,
    ) -> Result<(), DomainError>;

    async fn get_calendar_shares(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<Vec<(String, String)>, DomainError>;

    async fn create_event(
        &self,
        event: CreateEventDto,
        user_id: Uuid,
    ) -> Result<CalendarEventDto, DomainError>;

    async fn create_event_from_ical(
        &self,
        event: CreateEventICalDto,
        user_id: Uuid,
    ) -> Result<CalendarEventDto, DomainError>;

    async fn put_calendar_object(
        &self,
        put: PutCalendarObjectDto,
        user_id: Uuid,
    ) -> Result<CalendarObjectPutResultDto, DomainError>;

    async fn delete_calendar_object(
        &self,
        dto: DeleteCalendarObjectDto,
        user_id: Uuid,
    ) -> Result<CalendarObjectDeleteResultDto, DomainError>;

    async fn update_event(
        &self,
        event_id: &str,
        update: UpdateEventDto,
        user_id: Uuid,
    ) -> Result<CalendarEventDto, DomainError>;

    async fn delete_event(&self, event_id: &str, user_id: Uuid) -> Result<(), DomainError>;

    async fn get_event(
        &self,
        event_id: &str,
        user_id: Uuid,
    ) -> Result<CalendarEventDto, DomainError>;

    async fn get_event_by_resource_name(
        &self,
        calendar_id: &str,
        resource_name: &str,
        user_id: Uuid,
    ) -> Result<Option<CalendarEventDto>, DomainError>;

    async fn list_events(
        &self,
        calendar_id: &str,
        limit: Option<i64>,
        offset: Option<i64>,
        user_id: Uuid,
    ) -> Result<Vec<CalendarEventDto>, DomainError>;

    async fn get_events_in_range(
        &self,
        calendar_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        user_id: Uuid,
    ) -> Result<Vec<CalendarEventDto>, DomainError>;
}
