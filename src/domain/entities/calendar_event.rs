use chrono::{DateTime, Duration, TimeZone, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::common::errors::{DomainError, ErrorKind, Result};

pub use super::entity_errors::CalendarEventError;

#[derive(Debug, Clone)]
pub struct CalendarEvent {
    id: Uuid,
    calendar_id: Uuid,
    summary: String,
    description: Option<String>,
    location: Option<String>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    all_day: bool,
    rrule: Option<String>,
    ical_uid: String,
    resource_path: String,
    ical_data: String,
    etag: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl CalendarEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        calendar_id: Uuid,
        summary: String,
        description: Option<String>,
        location: Option<String>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        all_day: bool,
        rrule: Option<String>,
        ical_data: String,
    ) -> Result<Self> {
        if summary.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Event summary cannot be empty",
            ));
        }

        if end_time < start_time {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "End time cannot be before start time",
            ));
        }

        if let Some(ref rule) = rrule
            && !rule.starts_with("FREQ=")
        {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Recurrence rule must start with FREQ=",
            ));
        }

        Self::validate_ical_object(&ical_data)?;

        let ical_uid = Self::extract_ical_property(&ical_data, "UID")
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let resource_path = Self::default_resource_path_for_uid(&ical_uid);
        let etag = Self::generate_etag(&ical_data);
        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            calendar_id,
            summary,
            description,
            location,
            start_time,
            end_time,
            all_day,
            rrule,
            ical_uid,
            resource_path,
            ical_data,
            etag,
            created_at: now,
            updated_at: now,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_id(
        id: Uuid,
        calendar_id: Uuid,
        summary: String,
        description: Option<String>,
        location: Option<String>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        all_day: bool,
        rrule: Option<String>,
        ical_uid: String,
        resource_path: String,
        ical_data: String,
        etag: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self> {
        if summary.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Event summary cannot be empty",
            ));
        }

        if end_time < start_time {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "End time cannot be before start time",
            ));
        }

        if ical_uid.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "iCalendar UID cannot be empty",
            ));
        }

        Self::validate_resource_path(&resource_path)?;
        Self::validate_etag(&etag)?;
        Self::validate_ical_object(&ical_data)?;

        Ok(Self {
            id,
            calendar_id,
            summary,
            description,
            location,
            start_time,
            end_time,
            all_day,
            rrule,
            ical_uid,
            resource_path,
            ical_data,
            etag,
            created_at,
            updated_at,
        })
    }

    pub fn from_ical(calendar_id: Uuid, ical_data: String) -> Result<Self> {
        let uid = Self::extract_ical_property(&ical_data, "UID").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Missing UID in iCalendar data",
            )
        })?;

        Self::from_ical_with_resource_path(
            calendar_id,
            Self::default_resource_path_for_uid(&uid),
            ical_data,
        )
    }

    pub fn from_ical_with_resource_path(
        calendar_id: Uuid,
        resource_path: String,
        ical_data: String,
    ) -> Result<Self> {
        Self::validate_ical_object(&ical_data)?;
        Self::validate_resource_path(&resource_path)?;

        let summary = Self::extract_ical_property(&ical_data, "SUMMARY").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Missing SUMMARY in iCalendar data",
            )
        })?;

        let ical_uid = Self::extract_ical_property(&ical_data, "UID").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Missing UID in iCalendar data",
            )
        })?;

        let dtstart = Self::extract_ical_property(&ical_data, "DTSTART").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Missing DTSTART in iCalendar data",
            )
        })?;

        let dtend = Self::extract_ical_property(&ical_data, "DTEND").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Missing DTEND in iCalendar data",
            )
        })?;

        let start_time = Self::parse_ical_datetime(&dtstart).map_err(|e| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                format!("Invalid DTSTART: {}", e),
            )
        })?;

        let end_time = Self::parse_ical_datetime(&dtend).map_err(|e| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                format!("Invalid DTEND: {}", e),
            )
        })?;

        if end_time < start_time {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "End time cannot be before start time",
            ));
        }

        let description = Self::extract_ical_property(&ical_data, "DESCRIPTION");
        let location = Self::extract_ical_property(&ical_data, "LOCATION");
        let rrule = Self::extract_ical_property(&ical_data, "RRULE");
        let all_day = dtstart.contains("VALUE=DATE") || (!dtstart.contains('T') && dtstart.len() == 8);
        let etag = Self::generate_etag(&ical_data);
        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            calendar_id,
            summary,
            description,
            location,
            start_time,
            end_time,
            all_day,
            rrule,
            ical_uid,
            resource_path,
            ical_data,
            etag,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn calendar_id(&self) -> &Uuid {
        &self.calendar_id
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    pub fn start_time(&self) -> &DateTime<Utc> {
        &self.start_time
    }

    pub fn end_time(&self) -> &DateTime<Utc> {
        &self.end_time
    }

    pub fn all_day(&self) -> bool {
        self.all_day
    }

    pub fn rrule(&self) -> Option<&str> {
        self.rrule.as_deref()
    }

    pub fn ical_uid(&self) -> &str {
        &self.ical_uid
    }

    pub fn resource_path(&self) -> &str {
        &self.resource_path
    }

    pub fn ical_data(&self) -> &str {
        &self.ical_data
    }

    pub fn etag(&self) -> &str {
        &self.etag
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    pub fn duration(&self) -> Duration {
        self.end_time - self.start_time
    }

    pub fn update_summary(&mut self, summary: String) -> Result<()> {
        if summary.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Event summary cannot be empty",
            ));
        }

        self.summary = summary.clone();
        self.updated_at = Utc::now();
        self.update_ical_property("SUMMARY", &summary);
        self.refresh_etag();

        Ok(())
    }

    pub fn update_description(&mut self, description: Option<String>) {
        self.description = description.clone();
        self.updated_at = Utc::now();

        match description {
            Some(value) => self.update_ical_property("DESCRIPTION", &value),
            None => self.remove_ical_property("DESCRIPTION"),
        }

        self.refresh_etag();
    }

    pub fn update_location(&mut self, location: Option<String>) {
        self.location = location.clone();
        self.updated_at = Utc::now();

        match location {
            Some(value) => self.update_ical_property("LOCATION", &value),
            None => self.remove_ical_property("LOCATION"),
        }

        self.refresh_etag();
    }

    pub fn update_time_range(
        &mut self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<()> {
        if end_time < start_time {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "End time cannot be before start time",
            ));
        }

        self.start_time = start_time;
        self.end_time = end_time;
        self.updated_at = Utc::now();

        self.update_ical_property("DTSTART", &start_time.format("%Y%m%dT%H%M%SZ").to_string());
        self.update_ical_property("DTEND", &end_time.format("%Y%m%dT%H%M%SZ").to_string());
        self.refresh_etag();

        Ok(())
    }

    pub fn update_all_day(&mut self, all_day: bool) {
        self.all_day = all_day;
        self.updated_at = Utc::now();
        self.refresh_etag();
    }

    pub fn update_rrule(&mut self, rrule: Option<String>) -> Result<()> {
        if let Some(ref rule) = rrule
            && !rule.starts_with("FREQ=")
        {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Recurrence rule must start with FREQ=",
            ));
        }

        self.rrule = rrule.clone();
        self.updated_at = Utc::now();

        match rrule {
            Some(value) => self.update_ical_property("RRULE", &value),
            None => self.remove_ical_property("RRULE"),
        }

        self.refresh_etag();

        Ok(())
    }

    pub fn update_ical_data(&mut self, ical_data: String) -> Result<()> {
        Self::validate_ical_object(&ical_data)?;

        self.summary = Self::extract_ical_property(&ical_data, "SUMMARY").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Missing SUMMARY in iCalendar data",
            )
        })?;

        self.ical_uid = Self::extract_ical_property(&ical_data, "UID").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Missing UID in iCalendar data",
            )
        })?;

        let dtstart = Self::extract_ical_property(&ical_data, "DTSTART").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Missing DTSTART in iCalendar data",
            )
        })?;

        let dtend = Self::extract_ical_property(&ical_data, "DTEND").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Missing DTEND in iCalendar data",
            )
        })?;

        self.start_time = Self::parse_ical_datetime(&dtstart).map_err(|e| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                format!("Invalid DTSTART: {}", e),
            )
        })?;

        self.end_time = Self::parse_ical_datetime(&dtend).map_err(|e| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                format!("Invalid DTEND: {}", e),
            )
        })?;

        self.description = Self::extract_ical_property(&ical_data, "DESCRIPTION");
        self.location = Self::extract_ical_property(&ical_data, "LOCATION");
        self.rrule = Self::extract_ical_property(&ical_data, "RRULE");
        self.all_day = dtstart.contains("VALUE=DATE") || (!dtstart.contains('T') && dtstart.len() == 8);
        self.ical_data = ical_data;
        self.refresh_etag();

        Ok(())
    }

    pub fn update_resource_path(&mut self, resource_path: String) -> Result<()> {
        Self::validate_resource_path(&resource_path)?;
        self.resource_path = resource_path;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn refresh_etag(&mut self) {
        self.etag = Self::generate_etag(&self.ical_data);
        self.updated_at = Utc::now();
    }

    pub fn belongs_to_calendar(&self, calendar_id: &Uuid) -> bool {
        &self.calendar_id == calendar_id
    }

    pub fn occurs_in_range(&self, start: &DateTime<Utc>, end: &DateTime<Utc>) -> bool {
        self.start_time < *end && self.end_time > *start
    }

    fn validate_ical_object(ical_data: &str) -> Result<()> {
        if !ical_data.contains("BEGIN:VCALENDAR")
            || !ical_data.contains("END:VCALENDAR")
            || !ical_data.contains("BEGIN:VEVENT")
            || !ical_data.contains("END:VEVENT")
        {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "iCalendar data must contain a VCALENDAR with a VEVENT component",
            ));
        }

        if ical_data.contains("BEGIN:VTODO") || ical_data.contains("BEGIN:VJOURNAL") {
            return Err(DomainError::new(
                ErrorKind::UnsupportedOperation,
                "CalendarEvent",
                "Only VEVENT calendar object resources are supported",
            ));
        }

        Ok(())
    }

    fn extract_ical_property(ical_data: &str, property_name: &str) -> Option<String> {
        for raw in ical_data.replace("\r\n", "\n").lines() {
            let line = raw.trim();
            let (name, value) = line.split_once(':')?;
            let base_name = name.split(';').next().unwrap_or(name);

            if base_name.eq_ignore_ascii_case(property_name) && !value.trim().is_empty() {
                return Some(value.trim().to_string());
            }
        }

        None
    }

    fn parse_ical_datetime(datetime: &str) -> std::result::Result<DateTime<Utc>, String> {
        if datetime.contains("VALUE=DATE") {
            let date_str = datetime.split(':').next_back().unwrap_or("");
            return Self::parse_ical_date(date_str);
        }

        let value = datetime.split(':').next_back().unwrap_or(datetime);

        if value.len() == 8 {
            return Self::parse_ical_date(value);
        }

        if value.len() < 15 || !value.ends_with('Z') {
            return Err("Invalid datetime format".to_string());
        }

        let year = value[0..4]
            .parse::<i32>()
            .map_err(|_| "Invalid year".to_string())?;
        let month = value[4..6]
            .parse::<u32>()
            .map_err(|_| "Invalid month".to_string())?;
        let day = value[6..8]
            .parse::<u32>()
            .map_err(|_| "Invalid day".to_string())?;
        let hour = value[9..11]
            .parse::<u32>()
            .map_err(|_| "Invalid hour".to_string())?;
        let minute = value[11..13]
            .parse::<u32>()
            .map_err(|_| "Invalid minute".to_string())?;
        let second = value[13..15]
            .parse::<u32>()
            .map_err(|_| "Invalid second".to_string())?;

        let date = chrono::NaiveDate::from_ymd_opt(year, month, day)
            .ok_or_else(|| "Invalid date components".to_string())?;
        let datetime = date
            .and_hms_opt(hour, minute, second)
            .ok_or_else(|| "Invalid time components".to_string())?;

        Ok(Utc.from_utc_datetime(&datetime))
    }

    fn parse_ical_date(value: &str) -> std::result::Result<DateTime<Utc>, String> {
        if value.len() != 8 {
            return Err("Invalid date format".to_string());
        }

        let year = value[0..4]
            .parse::<i32>()
            .map_err(|_| "Invalid year".to_string())?;
        let month = value[4..6]
            .parse::<u32>()
            .map_err(|_| "Invalid month".to_string())?;
        let day = value[6..8]
            .parse::<u32>()
            .map_err(|_| "Invalid day".to_string())?;

        let date = chrono::NaiveDate::from_ymd_opt(year, month, day)
            .ok_or_else(|| "Invalid date components".to_string())?;

        Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()))
    }

    fn update_ical_property(&mut self, property_name: &str, value: &str) {
        let normalized = self.ical_data.replace("\r\n", "\n");
        let mut updated = Vec::new();
        let mut replaced = false;

        for line in normalized.lines() {
            let name = line
                .split_once(':')
                .map(|(name, _)| name.split(';').next().unwrap_or(name))
                .unwrap_or("");

            if name.eq_ignore_ascii_case(property_name) {
                updated.push(format!("{}:{}", property_name, value));
                replaced = true;
            } else if line == "END:VEVENT" && !replaced {
                updated.push(format!("{}:{}", property_name, value));
                updated.push(line.to_string());
                replaced = true;
            } else {
                updated.push(line.to_string());
            }
        }

        self.ical_data = updated.join("\r\n");
    }

    fn remove_ical_property(&mut self, property_name: &str) {
        let normalized = self.ical_data.replace("\r\n", "\n");

        self.ical_data = normalized
            .lines()
            .filter(|line| {
                let name = line
                    .split_once(':')
                    .map(|(name, _)| name.split(';').next().unwrap_or(name))
                    .unwrap_or("");

                !name.eq_ignore_ascii_case(property_name)
            })
            .collect::<Vec<_>>()
            .join("\r\n");
    }

    fn default_resource_path_for_uid(ical_uid: &str) -> String {
        format!("{}.ics", ical_uid.trim())
    }

    pub fn generate_etag(ical_data: &str) -> String {
        hex::encode(Sha256::digest(ical_data.as_bytes()))
    }

    fn validate_resource_path(resource_path: &str) -> Result<()> {
        if resource_path.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Resource path cannot be empty",
            ));
        }

        if resource_path.contains('/') || resource_path.chars().any(char::is_control) {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Resource path must be a single path segment",
            ));
        }

        Ok(())
    }

    fn validate_etag(etag: &str) -> Result<()> {
        if etag.trim().is_empty()
            || etag.contains('"')
            || etag.chars().any(char::is_control)
            || etag.starts_with("W/")
        {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "ETag must be a strong unquoted token",
            ));
        }

        Ok(())
    }
}
