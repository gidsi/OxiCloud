use crate::domain::entities::calendar::Calendar;
use crate::domain::entities::calendar_event::CalendarEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// DTO for calendar data transfer
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CalendarDto {
    pub id: String,
    pub name: String,
    pub path: String,
    pub owner_id: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub is_public: bool,
    pub ctag: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub custom_properties: HashMap<String, String>,
}

impl Default for CalendarDto {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            path: String::new(),
            owner_id: String::new(),
            description: None,
            color: None,
            is_public: false,
            ctag: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            custom_properties: HashMap::new(),
        }
    }
}

impl From<Calendar> for CalendarDto {
    fn from(calendar: Calendar) -> Self {
        Self {
            id: calendar.id().to_string(),
            name: calendar.name().to_string(),
            path: calendar.path().to_string(),
            owner_id: calendar.owner_id().to_string(),
            description: calendar.description().map(|s| s.to_string()),
            color: calendar.color().map(|s| s.to_string()),
            is_public: calendar.is_public(),
            ctag: calendar.ctag().to_string(),
            created_at: *calendar.created_at(),
            updated_at: *calendar.updated_at(),
            custom_properties: calendar.custom_properties().clone(),
        }
    }
}

/// DTO for calendar creation
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCalendarDto {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub is_public: Option<bool>,
}

/// DTO for calendar update
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCalendarDto {
    pub name: Option<String>,
    pub path: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub is_public: Option<bool>,
}

/// DTO for calendar sharing
#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarShareDto {
    pub calendar_id: String,
    pub user_id: String,
    pub access_level: String, // 'read', 'write', 'owner'
}

/// DTO for calendar event data transfer
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CalendarEventDto {
    pub id: String,
    pub calendar_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub all_day: bool,
    pub rrule: Option<String>,
    pub ical_uid: String,
    pub resource_path: String,
    pub ical_data: String,
    pub etag: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for CalendarEventDto {
    fn default() -> Self {
        Self {
            id: String::new(),
            calendar_id: String::new(),
            summary: String::new(),
            description: None,
            location: None,
            start_time: Utc::now(),
            end_time: Utc::now(),
            all_day: false,
            rrule: None,
            ical_uid: String::new(),
            resource_path: String::new(),
            ical_data: String::new(),
            etag: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl From<CalendarEvent> for CalendarEventDto {
    fn from(event: CalendarEvent) -> Self {
        Self {
            id: event.id().to_string(),
            calendar_id: event.calendar_id().to_string(),
            summary: event.summary().to_string(),
            description: event.description().map(|s| s.to_string()),
            location: event.location().map(|s| s.to_string()),
            start_time: *event.start_time(),
            end_time: *event.end_time(),
            all_day: event.all_day(),
            rrule: event.rrule().map(|s| s.to_string()),
            ical_uid: event.ical_uid().to_string(),
            resource_path: event.resource_path().to_string(),
            ical_data: event.ical_data().to_string(),
            etag: event.etag().to_string(),
            created_at: *event.created_at(),
            updated_at: *event.updated_at(),
        }
    }
}

/// DTO for calendar event creation using iCalendar data
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventICalDto {
    pub calendar_id: String,
    #[serde(default)]
    pub resource_path: Option<String>,
    pub ical_data: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum EventPutPreconditionDto {
    None,
    IfMatch(String),
    IfMatchAny,
    IfNoneMatchAny,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PutEventICalDto {
    pub calendar_id: String,
    pub resource_path: String,
    pub ical_data: String,
    pub precondition: EventPutPreconditionDto,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PutEventICalResultDto {
    pub event: CalendarEventDto,
    pub created: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum EventDeletePreconditionDto {
    None,
    IfMatch(String),
    IfMatchAny,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteEventResourceDto {
    pub calendar_id: String,
    pub resource_path: String,
    pub precondition: EventDeletePreconditionDto,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum CalendarDeletePreconditionDto {
    None,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeleteCalendarCollectionDto {
    pub calendar_id: String,
    pub precondition: CalendarDeletePreconditionDto,
}

/// DTO for calendar event creation with structured data
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventDto {
    pub calendar_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub all_day: Option<bool>,
    pub rrule: Option<String>,
    pub ical_data: Option<String>,
    pub resource_path: Option<String>,
    pub expected_etag: Option<String>,
    pub user_id: String, // Added for authorization
}

/// DTO for updating a calendar event
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateEventDto {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub all_day: Option<bool>,
    pub rrule: Option<String>,
    pub ical_data: Option<String>,
    pub resource_path: Option<String>,
    pub expected_etag: Option<String>,
    pub user_id: String, // Added for authorization
}

/// DTO for querying events in a time range
#[derive(Debug, Serialize, Deserialize)]
pub struct EventQueryDto {
    pub calendar_id: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// DTO for pagination
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationDto {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
