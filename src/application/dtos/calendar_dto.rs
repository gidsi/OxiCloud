use crate::domain::entities::calendar::Calendar;
use crate::domain::entities::calendar_event::CalendarEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CalendarDto {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub owner_id: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub is_public: bool,
    pub ctag: String,
    pub sync_version: i64,
    pub supported_components: Vec<String>,
    pub timezone: Option<String>,
    pub calendar_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub custom_properties: HashMap<String, String>,
}

impl Default for CalendarDto {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            display_name: String::new(),
            owner_id: String::new(),
            description: None,
            color: None,
            is_public: false,
            ctag: "1".to_string(),
            sync_version: 1,
            supported_components: vec!["VEVENT".to_string(), "VTODO".to_string()],
            timezone: None,
            calendar_order: 0,
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
            display_name: calendar.display_name().to_string(),
            owner_id: calendar.owner_id().to_string(),
            description: calendar.description().map(|s| s.to_string()),
            color: calendar.color().map(|s| s.to_string()),
            is_public: calendar.is_public(),
            ctag: calendar.ctag().to_string(),
            sync_version: calendar.sync_version(),
            supported_components: calendar.supported_components().to_vec(),
            timezone: calendar.timezone().map(|s| s.to_string()),
            calendar_order: calendar.calendar_order(),
            created_at: *calendar.created_at(),
            updated_at: *calendar.updated_at(),
            custom_properties: calendar.custom_properties().clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCalendarDto {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub is_public: Option<bool>,
    #[serde(default)]
    pub supported_components: Option<Vec<String>>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub calendar_order: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCalendarDto {
    pub name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub color: Option<String>,
    pub is_public: Option<bool>,
    #[serde(default)]
    pub supported_components: Option<Vec<String>>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub calendar_order: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CalendarShareDto {
    pub calendar_id: String,
    pub user_id: String,
    pub access_level: String,
}

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
            created_at: *event.created_at(),
            updated_at: *event.updated_at(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventICalDto {
    pub calendar_id: String,
    pub ical_data: String,
}

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
    pub user_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateEventDto {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub all_day: Option<bool>,
    pub rrule: Option<String>,
    pub user_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventQueryDto {
    pub calendar_id: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationDto {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
