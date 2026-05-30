#![allow(clippy::collapsible_if)]
use chrono::{DateTime, Duration, TimeZone, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::common::errors::{DomainError, ErrorKind, Result};

pub use super::entity_errors::CalendarEventError;

#[derive(Debug, Clone)]
pub struct CalendarEvent {
    id: Uuid,
    calendar_id: Uuid,
    resource_name: String,
    summary: String,
    description: Option<String>,
    location: Option<String>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    all_day: bool,
    rrule: Option<String>,
    ical_uid: String,
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
        Self::new_with_resource_name(
            calendar_id,
            None,
            summary,
            description,
            location,
            start_time,
            end_time,
            all_day,
            rrule,
            ical_data,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_resource_name(
        calendar_id: Uuid,
        resource_name: Option<String>,
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

        if let Some(ref rule) = rrule {
            if !rule.starts_with("FREQ=") {
                return Err(DomainError::new(
                    ErrorKind::InvalidInput,
                    "CalendarEvent",
                    "Recurrence rule must start with FREQ=",
                ));
            }
        }

        Self::validate_calendar_object_data(&ical_data)?;

        let ical_uid = Self::extract_ical_property(&ical_data, "UID").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "iCalendar data must contain a UID property",
            )
        })?;

        let resource_name = resource_name.unwrap_or_else(|| Self::default_resource_name(&ical_uid));
        Self::validate_resource_name(&resource_name)?;

        let now = Utc::now();
        let id = Uuid::new_v4();
        let etag = Self::generate_etag(&id, &resource_name, &ical_data, &now);

        Ok(Self {
            id,
            calendar_id,
            resource_name,
            summary,
            description,
            location,
            start_time,
            end_time,
            all_day,
            rrule,
            ical_uid,
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
        ical_data: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self> {
        let resource_name = Self::default_resource_name(&ical_uid);
        let etag = Self::generate_etag(&id, &resource_name, &ical_data, &updated_at);

        Self::with_id_and_metadata(
            id,
            calendar_id,
            resource_name,
            summary,
            description,
            location,
            start_time,
            end_time,
            all_day,
            rrule,
            ical_uid,
            ical_data,
            etag,
            created_at,
            updated_at,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_id_and_metadata(
        id: Uuid,
        calendar_id: Uuid,
        resource_name: String,
        summary: String,
        description: Option<String>,
        location: Option<String>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        all_day: bool,
        rrule: Option<String>,
        ical_uid: String,
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

        if ical_data.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "iCalendar data cannot be empty",
            ));
        }

        Self::validate_resource_name(&resource_name)?;

        if etag.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "ETag cannot be empty",
            ));
        }

        Ok(Self {
            id,
            calendar_id,
            resource_name,
            summary,
            description,
            location,
            start_time,
            end_time,
            all_day,
            rrule,
            ical_uid,
            ical_data,
            etag,
            created_at,
            updated_at,
        })
    }

    pub fn from_ical(calendar_id: Uuid, ical_data: String) -> Result<Self> {
        Self::from_ical_with_resource_name(calendar_id, None, ical_data)
    }

    pub fn from_ical_with_resource_name(
        calendar_id: Uuid,
        resource_name: Option<String>,
        ical_data: String,
    ) -> Result<Self> {
        Self::validate_calendar_object_data(&ical_data)?;

        let summary = Self::extract_ical_property(&ical_data, "SUMMARY")
            .unwrap_or_else(|| "Untitled event".to_string());

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

        let all_day = dtstart.contains("VALUE=DATE") && !dtstart.contains('T');
        let description = Self::extract_ical_property(&ical_data, "DESCRIPTION");
        let location = Self::extract_ical_property(&ical_data, "LOCATION");
        let rrule = Self::extract_ical_property(&ical_data, "RRULE");
        let ical_uid = Self::extract_ical_property(&ical_data, "UID").ok_or_else(|| {
            DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "Missing UID in iCalendar data",
            )
        })?;

        let resource_name = resource_name.unwrap_or_else(|| Self::default_resource_name(&ical_uid));
        Self::validate_resource_name(&resource_name)?;

        let now = Utc::now();
        let id = Uuid::new_v4();
        let etag = Self::generate_etag(&id, &resource_name, &ical_data, &now);

        Ok(Self {
            id,
            calendar_id,
            resource_name,
            summary,
            description,
            location,
            start_time,
            end_time,
            all_day,
            rrule,
            ical_uid,
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

    pub fn resource_name(&self) -> &str {
        &self.resource_name
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

    pub fn ical_data(&self) -> &str {
        &self.ical_data
    }

    pub fn etag(&self) -> &str {
        &self.etag
    }

    pub fn etag_matches(&self, etag: &str) -> bool {
        self.etag == etag
    }

    pub fn etag_matches_any(&self, etags: &[String]) -> bool {
        etags.iter().any(|etag| self.etag_matches(etag))
    }

    pub fn quoted_etag(&self) -> String {
        format!("\"{}\"", self.etag)
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
            Some(desc) => self.update_ical_property("DESCRIPTION", &desc),
            None => self.remove_ical_property("DESCRIPTION"),
        }

        self.refresh_etag();
    }

    pub fn update_location(&mut self, location: Option<String>) {
        self.location = location.clone();
        self.updated_at = Utc::now();

        match location {
            Some(loc) => self.update_ical_property("LOCATION", &loc),
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

        let start_str = if self.all_day {
            format!("{}T000000Z", start_time.format("%Y%m%d"))
        } else {
            start_time.format("%Y%m%dT%H%M%SZ").to_string()
        };

        let end_str = if self.all_day {
            format!("{}T000000Z", end_time.format("%Y%m%d"))
        } else {
            end_time.format("%Y%m%dT%H%M%SZ").to_string()
        };

        self.update_ical_property("DTSTART", &start_str);
        self.update_ical_property("DTEND", &end_str);
        self.refresh_etag();

        Ok(())
    }

    pub fn update_all_day(&mut self, all_day: bool) {
        self.all_day = all_day;
        self.updated_at = Utc::now();

        let start_str = if all_day {
            format!("VALUE=DATE:{}", self.start_time.format("%Y%m%d"))
        } else {
            self.start_time.format("%Y%m%dT%H%M%SZ").to_string()
        };

        let end_str = if all_day {
            format!("VALUE=DATE:{}", self.end_time.format("%Y%m%d"))
        } else {
            self.end_time.format("%Y%m%dT%H%M%SZ").to_string()
        };

        self.update_ical_property("DTSTART", &start_str);
        self.update_ical_property("DTEND", &end_str);
        self.refresh_etag();
    }

    pub fn update_rrule(&mut self, rrule: Option<String>) -> Result<()> {
        if let Some(ref rule) = rrule {
            if !rule.starts_with("FREQ=") {
                return Err(DomainError::new(
                    ErrorKind::InvalidInput,
                    "CalendarEvent",
                    "Recurrence rule must start with FREQ=",
                ));
            }
        }

        self.rrule = rrule.clone();
        self.updated_at = Utc::now();

        match rrule {
            Some(rule) => self.update_ical_property("RRULE", &rule),
            None => self.remove_ical_property("RRULE"),
        }

        self.refresh_etag();

        Ok(())
    }

    pub fn update_ical_data(&mut self, ical_data: String) -> Result<()> {
        Self::validate_calendar_object_data(&ical_data)?;

        if let Some(summary) = Self::extract_ical_property(&ical_data, "SUMMARY") {
            self.summary = summary;
        }

        self.description = Self::extract_ical_property(&ical_data, "DESCRIPTION");
        self.location = Self::extract_ical_property(&ical_data, "LOCATION");

        if let Some(dtstart) = Self::extract_ical_property(&ical_data, "DTSTART") {
            self.start_time = Self::parse_ical_datetime(&dtstart).map_err(|e| {
                DomainError::new(
                    ErrorKind::InvalidInput,
                    "CalendarEvent",
                    format!("Invalid DTSTART: {}", e),
                )
            })?;
            self.all_day = dtstart.contains("VALUE=DATE") && !dtstart.contains('T');
        }

        if let Some(dtend) = Self::extract_ical_property(&ical_data, "DTEND") {
            self.end_time = Self::parse_ical_datetime(&dtend).map_err(|e| {
                DomainError::new(
                    ErrorKind::InvalidInput,
                    "CalendarEvent",
                    format!("Invalid DTEND: {}", e),
                )
            })?;
        }

        self.rrule = Self::extract_ical_property(&ical_data, "RRULE");

        if let Some(uid) = Self::extract_ical_property(&ical_data, "UID") {
            self.ical_uid = uid;
        }

        self.ical_data = ical_data;
        self.updated_at = Utc::now();
        self.refresh_etag();

        Ok(())
    }

    pub fn update_resource_name(&mut self, resource_name: String) -> Result<()> {
        Self::validate_resource_name(&resource_name)?;
        self.resource_name = resource_name;
        self.updated_at = Utc::now();
        self.refresh_etag();
        Ok(())
    }

    pub fn belongs_to_calendar(&self, calendar_id: &Uuid) -> bool {
        self.calendar_id == *calendar_id
    }

    pub fn occurs_in_range(&self, start: &DateTime<Utc>, end: &DateTime<Utc>) -> bool {
        if self.start_time <= *end && self.end_time >= *start {
            return true;
        }

        if let Some(rrule) = &self.rrule {
            if let Some(until_pos) = rrule.find("UNTIL=") {
                let until_start = until_pos + 6;

                if let Some(until_end) = rrule[until_start..].find(';') {
                    let until_str = &rrule[until_start..until_start + until_end];
                    if let Ok(until_date) = Self::parse_ical_datetime(until_str) {
                        return until_date >= *start;
                    }
                } else {
                    let until_str = &rrule[until_start..];
                    if let Ok(until_date) = Self::parse_ical_datetime(until_str) {
                        return until_date >= *start;
                    }
                }
            } else {
                return true;
            }
        }

        false
    }

    fn extract_ical_property(ical_data: &str, property_name: &str) -> Option<String> {
        for raw_line in ical_data.lines() {
            let line = raw_line.trim_end_matches('\r');

            let Some(colon_pos) = line.find(':') else {
                continue;
            };

            let name_and_params = &line[..colon_pos];
            let name = name_and_params.split(';').next().unwrap_or_default();

            if name.eq_ignore_ascii_case(property_name) {
                let value = line[colon_pos + 1..].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }

        None
    }

    fn validate_calendar_object_data(ical_data: &str) -> Result<()> {
        let trimmed_start = ical_data.trim_start();
        let trimmed_end = ical_data.trim_end();

        if trimmed_end.is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "iCalendar data cannot be empty",
            ));
        }

        if !trimmed_start.starts_with("BEGIN:VCALENDAR") {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "iCalendar data must start with BEGIN:VCALENDAR",
            ));
        }

        if !trimmed_end.ends_with("END:VCALENDAR") {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "iCalendar data must end with END:VCALENDAR",
            ));
        }

        if !ical_data
            .lines()
            .any(|line| line.trim_end_matches('\r') == "VERSION:2.0")
        {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "iCalendar data must contain VERSION:2.0",
            ));
        }

        if !ical_data.contains("BEGIN:VEVENT") || !ical_data.contains("END:VEVENT") {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "iCalendar data must contain a VEVENT component",
            ));
        }

        if Self::extract_ical_property(ical_data, "UID")
            .map(|uid| uid.trim().is_empty())
            .unwrap_or(true)
        {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "iCalendar data must contain a UID property",
            ));
        }

        Ok(())
    }

    fn validate_resource_name(resource_name: &str) -> Result<()> {
        if resource_name.trim().is_empty()
            || resource_name.contains('/')
            || resource_name.contains('\\')
            || resource_name == "."
            || resource_name == ".."
            || !resource_name.to_ascii_lowercase().ends_with(".ics")
        {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "CalendarEvent",
                "CalDAV resource name must be a non-empty .ics file name without path separators",
            ));
        }

        Ok(())
    }

    fn default_resource_name(ical_uid: &str) -> String {
        let sanitized: String = ical_uid
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '~' | '-') {
                    ch
                } else {
                    '-'
                }
            })
            .collect();

        let trimmed = sanitized.trim_matches(['.', '-']).to_string();
        let base = if trimmed.is_empty() {
            format!("event-{}", Uuid::new_v4())
        } else {
            trimmed
        };

        if base.to_ascii_lowercase().ends_with(".ics") {
            base
        } else {
            format!("{}.ics", base)
        }
    }

    fn generate_etag(
        id: &Uuid,
        resource_name: &str,
        ical_data: &str,
        updated_at: &DateTime<Utc>,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(id.as_bytes());
        hasher.update(b":");
        hasher.update(resource_name.as_bytes());
        hasher.update(b":");
        hasher.update(ical_data.as_bytes());
        hasher.update(b":");
        hasher.update(updated_at.timestamp_micros().to_string().as_bytes());
        hex::encode(hasher.finalize())
    }

    fn refresh_etag(&mut self) {
        self.etag = Self::generate_etag(
            &self.id,
            &self.resource_name,
            &self.ical_data,
            &self.updated_at,
        );
    }

    fn parse_ical_datetime(datetime: &str) -> std::result::Result<DateTime<Utc>, String> {
        if datetime.contains("VALUE=DATE") {
            let date_str = datetime.split(':').next_back().unwrap_or("");
            if date_str.len() != 8 {
                return Err("Invalid date format".to_string());
            }

            let year = date_str[0..4]
                .parse::<i32>()
                .map_err(|_| "Invalid year".to_string())?;
            let month = date_str[4..6]
                .parse::<u32>()
                .map_err(|_| "Invalid month".to_string())?;
            let day = date_str[6..8]
                .parse::<u32>()
                .map_err(|_| "Invalid day".to_string())?;

            return match chrono::NaiveDate::from_ymd_opt(year, month, day) {
                Some(date) => Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap())),
                None => Err("Invalid date components".to_string()),
            };
        }

        let datetime_str = datetime.split(':').next_back().unwrap_or(datetime);
        if datetime_str.len() < 15 || !datetime_str.ends_with('Z') {
            return Err("Invalid datetime format".to_string());
        }

        let year = datetime_str[0..4]
            .parse::<i32>()
            .map_err(|_| "Invalid year".to_string())?;
        let month = datetime_str[4..6]
            .parse::<u32>()
            .map_err(|_| "Invalid month".to_string())?;
        let day = datetime_str[6..8]
            .parse::<u32>()
            .map_err(|_| "Invalid day".to_string())?;
        let hour = datetime_str[9..11]
            .parse::<u32>()
            .map_err(|_| "Invalid hour".to_string())?;
        let minute = datetime_str[11..13]
            .parse::<u32>()
            .map_err(|_| "Invalid minute".to_string())?;
        let second = datetime_str[13..15]
            .parse::<u32>()
            .map_err(|_| "Invalid second".to_string())?;

        match chrono::NaiveDate::from_ymd_opt(year, month, day) {
            Some(date) => match date.and_hms_opt(hour, minute, second) {
                Some(datetime) => Ok(Utc.from_utc_datetime(&datetime)),
                None => Err("Invalid time components".to_string()),
            },
            None => Err("Invalid date components".to_string()),
        }
    }

    fn update_ical_property(&mut self, property_name: &str, value: &str) {
        let mut lines: Vec<String> = self
            .ical_data
            .lines()
            .map(|line| line.trim_end_matches('\r').to_string())
            .collect();

        let mut found = false;

        for line in &mut lines {
            let Some(colon_pos) = line.find(':') else {
                continue;
            };

            let name = line[..colon_pos].split(';').next().unwrap_or_default();
            if name.eq_ignore_ascii_case(property_name) {
                *line = format!("{}:{}", property_name, value);
                found = true;
                break;
            }
        }

        if !found {
            if let Some(pos) = lines.iter().position(|line| line == "END:VEVENT") {
                lines.insert(pos, format!("{}:{}", property_name, value));
            }
        }

        self.ical_data = lines.join("\r\n");
        if !self.ical_data.ends_with("\r\n") {
            self.ical_data.push_str("\r\n");
        }
    }

    fn remove_ical_property(&mut self, property_name: &str) {
        self.ical_data = self
            .ical_data
            .lines()
            .filter(|line| {
                let line = line.trim_end_matches('\r');
                let Some(colon_pos) = line.find(':') else {
                    return true;
                };

                let name = line[..colon_pos].split(';').next().unwrap_or_default();
                !name.eq_ignore_ascii_case(property_name)
            })
            .collect::<Vec<_>>()
            .join("\r\n");

        if !self.ical_data.ends_with("\r\n") {
            self.ical_data.push_str("\r\n");
        }
    }
}
