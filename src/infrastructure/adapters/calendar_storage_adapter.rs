//! Calendar Storage Adapter
//!
//! This adapter implements the `CalendarStoragePort` application port using
//! the `CalendarRepository` and `CalendarEventRepository` domain repositories.
//! It bridges the gap between the application layer and the infrastructure layer.

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::application::dtos::calendar_dto::{
    CalendarDto, CalendarEventDto, CalendarObjectPutConditionDto, CalendarObjectPutResultDto,
    CalendarObjectPutStatusDto, CreateCalendarDto, CreateEventDto, CreateEventICalDto,
    PutCalendarObjectDto, UpdateCalendarDto, UpdateEventDto,
};
use crate::application::ports::calendar_ports::CalendarStoragePort;
use crate::common::errors::{DomainError, ErrorKind};
use crate::domain::entities::calendar::Calendar;
use crate::domain::entities::calendar_event::CalendarEvent;
use crate::domain::repositories::calendar_event_repository::{
    CalendarEventReplaceResult, CalendarEventRepository,
};
use crate::domain::repositories::calendar_repository::CalendarRepository;
use crate::infrastructure::repositories::pg::CalendarEventPgRepository;
use crate::infrastructure::repositories::pg::CalendarPgRepository;

/// Adapter that implements CalendarStoragePort using domain repositories
pub struct CalendarStorageAdapter {
    calendar_repository: Arc<CalendarPgRepository>,
    event_repository: Arc<CalendarEventPgRepository>,
}

impl CalendarStorageAdapter {
    /// Creates a new CalendarStorageAdapter with the given repositories
    pub fn new(
        calendar_repository: Arc<CalendarPgRepository>,
        event_repository: Arc<CalendarEventPgRepository>,
    ) -> Self {
        Self {
            calendar_repository,
            event_repository,
        }
    }
}

impl CalendarStoragePort for CalendarStorageAdapter {
    async fn create_calendar(
        &self,
        dto: CreateCalendarDto,
        owner_id: Uuid,
    ) -> Result<CalendarDto, DomainError> {
        let calendar = match dto.slug {
            Some(slug) => Calendar::new_with_slug(
                dto.name,
                slug,
                owner_id,
                dto.description,
                dto.color,
                dto.is_public.unwrap_or(false),
                None,
                HashMap::new(),
            )?,
            None => Calendar::new(dto.name, owner_id, dto.description, dto.color)?,
        };

        let created = self.calendar_repository.create_calendar(calendar).await?;
        Ok(CalendarDto::from(created))
    }

    async fn update_calendar(
        &self,
        calendar_id: &str,
        update: UpdateCalendarDto,
    ) -> Result<CalendarDto, DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        let mut calendar = self.calendar_repository.find_calendar_by_id(&uuid).await?;

        if let Some(slug) = update.slug {
            calendar.update_slug(slug)?;
        }
        if let Some(name) = update.name {
            calendar.update_name(name)?;
        }
        if let Some(description) = update.description {
            calendar.update_description(Some(description));
        }
        if let Some(color) = update.color {
            calendar.update_color(Some(color))?;
        }
        if let Some(is_public) = update.is_public {
            calendar.update_public_visibility(is_public);
        }

        let updated = self.calendar_repository.update_calendar(calendar).await?;
        Ok(CalendarDto::from(updated))
    }

    async fn delete_calendar(&self, calendar_id: &str) -> Result<(), DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        self.event_repository
            .delete_all_events_in_calendar(&uuid)
            .await?;

        self.calendar_repository.delete_calendar(&uuid).await
    }

    async fn get_calendar(&self, calendar_id: &str) -> Result<CalendarDto, DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        let calendar = self.calendar_repository.find_calendar_by_id(&uuid).await?;
        Ok(CalendarDto::from(calendar))
    }

    async fn list_calendars_by_owner(
        &self,
        owner_id: Uuid,
    ) -> Result<Vec<CalendarDto>, DomainError> {
        let calendars = self
            .calendar_repository
            .list_calendars_by_owner(owner_id)
            .await?;

        Ok(calendars.into_iter().map(CalendarDto::from).collect())
    }

    async fn list_calendars_shared_with_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<CalendarDto>, DomainError> {
        let calendars = self
            .calendar_repository
            .list_calendars_shared_with_user(user_id)
            .await?;

        Ok(calendars.into_iter().map(CalendarDto::from).collect())
    }

    async fn list_public_calendars(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CalendarDto>, DomainError> {
        let calendars = self
            .calendar_repository
            .list_public_calendars(limit, offset)
            .await?;

        Ok(calendars.into_iter().map(CalendarDto::from).collect())
    }

    async fn check_calendar_access(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<bool, DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        self.calendar_repository
            .user_has_calendar_access(&uuid, user_id)
            .await
    }

    async fn find_calendar_by_slug_for_owner(
        &self,
        slug: &str,
        owner_id: Uuid,
    ) -> Result<CalendarDto, DomainError> {
        let calendar = self
            .calendar_repository
            .find_calendar_by_slug_and_owner(slug, owner_id)
            .await?;
        Ok(CalendarDto::from(calendar))
    }

    async fn share_calendar(
        &self,
        calendar_id: &str,
        user_id: Uuid,
        access_level: &str,
    ) -> Result<(), DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        self.calendar_repository
            .share_calendar(&uuid, user_id, access_level)
            .await
    }

    async fn remove_calendar_sharing(
        &self,
        calendar_id: &str,
        user_id: Uuid,
    ) -> Result<(), DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        self.calendar_repository
            .remove_calendar_sharing(&uuid, user_id)
            .await
    }

    async fn get_calendar_shares(
        &self,
        calendar_id: &str,
    ) -> Result<Vec<(String, String)>, DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        self.calendar_repository.get_calendar_shares(&uuid).await
    }

    async fn set_calendar_property(
        &self,
        calendar_id: &str,
        property_name: &str,
        property_value: &str,
    ) -> Result<(), DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        self.calendar_repository
            .set_calendar_property(&uuid, property_name, property_value)
            .await
    }

    async fn get_calendar_property(
        &self,
        calendar_id: &str,
        property_name: &str,
    ) -> Result<Option<String>, DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        self.calendar_repository
            .get_calendar_property(&uuid, property_name)
            .await
    }

    async fn get_calendar_properties(
        &self,
        calendar_id: &str,
    ) -> Result<HashMap<String, String>, DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        self.calendar_repository
            .get_calendar_properties(&uuid)
            .await
    }

    async fn create_event(&self, dto: CreateEventDto) -> Result<CalendarEventDto, DomainError> {
        let calendar_id = Uuid::parse_str(&dto.calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Event",
                "Invalid calendar ID format",
            )
        })?;

        let _calendar = self
            .calendar_repository
            .find_calendar_by_id(&calendar_id)
            .await?;

        let ical_data = format!(
            "BEGIN:VCALENDAR\nVERSION:2.0\nPRODID:-//OxiCloud//EN\nBEGIN:VEVENT\nUID:{}@oxicloud\nDTSTAMP:{}\nDTSTART:{}\nDTEND:{}\nSUMMARY:{}\nEND:VEVENT\nEND:VCALENDAR",
            uuid::Uuid::new_v4(),
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
            dto.start_time.format("%Y%m%dT%H%M%SZ"),
            dto.end_time.format("%Y%m%dT%H%M%SZ"),
            dto.summary
        );

        let event = CalendarEvent::new(
            calendar_id,
            dto.summary,
            dto.description,
            dto.location,
            dto.start_time,
            dto.end_time,
            dto.all_day.unwrap_or(false),
            dto.rrule,
            ical_data,
        )?;

        let created = self.event_repository.create_event(event).await?;
        Ok(CalendarEventDto::from(created))
    }

    async fn create_event_from_ical(
        &self,
        dto: CreateEventICalDto,
    ) -> Result<CalendarEventDto, DomainError> {
        let calendar_id = Uuid::parse_str(&dto.calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Event",
                "Invalid calendar ID format",
            )
        })?;

        let _calendar = self
            .calendar_repository
            .find_calendar_by_id(&calendar_id)
            .await?;

        let event = CalendarEvent::from_ical_with_resource_name(
            calendar_id,
            dto.resource_name,
            dto.ical_data,
        )?;

        let created = self.event_repository.create_event(event).await?;
        Ok(CalendarEventDto::from(created))
    }

    async fn update_event(
        &self,
        event_id: &str,
        update: UpdateEventDto,
    ) -> Result<CalendarEventDto, DomainError> {
        let uuid = Uuid::parse_str(event_id).map_err(|_| {
            DomainError::new(ErrorKind::InvalidInput, "Event", "Invalid event ID format")
        })?;

        let mut event = self.event_repository.find_event_by_id(&uuid).await?;

        if let Some(summary) = update.summary {
            event.update_summary(summary)?;
        }
        if let Some(description) = update.description {
            event.update_description(Some(description));
        }
        if let Some(location) = update.location {
            event.update_location(Some(location));
        }
        if let Some(start_time) = update.start_time {
            if let Some(end_time) = update.end_time {
                event.update_time_range(start_time, end_time)?;
            } else {
                event.update_time_range(start_time, *event.end_time())?;
            }
        } else if let Some(end_time) = update.end_time {
            event.update_time_range(*event.start_time(), end_time)?;
        }
        if let Some(all_day) = update.all_day {
            event.update_all_day(all_day);
        }
        if let Some(rrule) = update.rrule {
            event.update_rrule(Some(rrule))?;
        }

        let updated = self.event_repository.update_event(event).await?;
        Ok(CalendarEventDto::from(updated))
    }

    async fn delete_event(&self, event_id: &str) -> Result<(), DomainError> {
        let uuid = Uuid::parse_str(event_id).map_err(|_| {
            DomainError::new(ErrorKind::InvalidInput, "Event", "Invalid event ID format")
        })?;

        self.event_repository.delete_event(&uuid).await
    }

    async fn get_event(&self, event_id: &str) -> Result<CalendarEventDto, DomainError> {
        let uuid = Uuid::parse_str(event_id).map_err(|_| {
            DomainError::new(ErrorKind::InvalidInput, "Event", "Invalid event ID format")
        })?;

        let event = self.event_repository.find_event_by_id(&uuid).await?;
        Ok(CalendarEventDto::from(event))
    }

    async fn list_events_by_calendar(
        &self,
        calendar_id: &str,
    ) -> Result<Vec<CalendarEventDto>, DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        let events = self.event_repository.list_events_by_calendar(&uuid).await?;
        Ok(events.into_iter().map(CalendarEventDto::from).collect())
    }

    async fn list_events_by_calendar_paginated(
        &self,
        calendar_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CalendarEventDto>, DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        let events = self
            .event_repository
            .list_events_by_calendar_paginated(&uuid, limit, offset)
            .await?;

        Ok(events.into_iter().map(CalendarEventDto::from).collect())
    }

    async fn put_calendar_object(
        &self,
        dto: PutCalendarObjectDto,
    ) -> Result<CalendarObjectPutResultDto, DomainError> {
        let calendar_id = Uuid::parse_str(&dto.calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarObject",
                "Invalid calendar ID format",
            )
        })?;

        let _calendar = self
            .calendar_repository
            .find_calendar_by_id(&calendar_id)
            .await?;

        let event = CalendarEvent::from_ical_with_resource_name(
            calendar_id,
            Some(dto.resource_name.clone()),
            dto.ical_data,
        )?;

        if let Some(conflict) = self
            .event_repository
            .find_uid_conflict(&calendar_id, event.ical_uid(), event.resource_name())
            .await?
        {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalDavUidConflict",
                format!(
                    "Another calendar object resource with the same UID already exists: {}",
                    conflict.resource_name()
                ),
            ));
        }

        match dto.condition {
            CalendarObjectPutConditionDto::IfNoneMatchAny => {
                let created = self
                    .event_repository
                    .create_event_if_resource_absent(event)
                    .await
                    .map_err(|e| {
                        if e.kind == ErrorKind::AlreadyExists {
                            DomainError::new(
                                ErrorKind::InvalidInput,
                                "CalDavPreconditionFailed",
                                "If-None-Match precondition failed because the resource already exists.",
                            )
                        } else {
                            e
                        }
                    })?;
                Ok(CalendarObjectPutResultDto {
                    status: CalendarObjectPutStatusDto::Created,
                    event: CalendarEventDto::from(created),
                })
            }
            CalendarObjectPutConditionDto::IfMatch(expected_etag) => match self
                .event_repository
                .replace_event_by_resource_name_and_etag(event, &expected_etag)
                .await?
            {
                CalendarEventReplaceResult::Replaced(updated) => Ok(CalendarObjectPutResultDto {
                    status: CalendarObjectPutStatusDto::Updated,
                    event: CalendarEventDto::from(updated),
                }),
                CalendarEventReplaceResult::PreconditionFailed => Err(DomainError::new(
                    ErrorKind::InvalidInput,
                    "CalDavPreconditionFailed",
                    "If-Match precondition failed because the supplied ETag does not match the current resource ETag.",
                )),
            },
            CalendarObjectPutConditionDto::None => {
                let existing = self
                    .event_repository
                    .find_event_by_resource_name(&calendar_id, event.resource_name())
                    .await?;
                if existing.is_some() {
                    let updated = self
                        .event_repository
                        .replace_event_by_resource_name(event)
                        .await?;
                    Ok(CalendarObjectPutResultDto {
                        status: CalendarObjectPutStatusDto::Updated,
                        event: CalendarEventDto::from(updated),
                    })
                } else {
                    let created = self.event_repository.create_event(event).await?;
                    Ok(CalendarObjectPutResultDto {
                        status: CalendarObjectPutStatusDto::Created,
                        event: CalendarEventDto::from(created),
                    })
                }
            }
        }
    }

    async fn get_event_by_resource_name(
        &self,
        calendar_id: &str,
        resource_name: &str,
    ) -> Result<Option<CalendarEventDto>, DomainError> {
        let calendar_id = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;
        let event = self
            .event_repository
            .find_event_by_resource_name(&calendar_id, resource_name)
            .await?;
        Ok(event.map(CalendarEventDto::from))
    }

    async fn get_events_in_time_range(
        &self,
        calendar_id: &str,
        start: &DateTime<Utc>,
        end: &DateTime<Utc>,
    ) -> Result<Vec<CalendarEventDto>, DomainError> {
        let uuid = Uuid::parse_str(calendar_id).map_err(|_| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Invalid calendar ID format",
            )
        })?;

        let events = self
            .event_repository
            .get_events_in_time_range(&uuid, start, end)
            .await?;

        Ok(events.into_iter().map(CalendarEventDto::from).collect())
    }
}
