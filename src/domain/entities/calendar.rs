use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

/**
 * Calendar Entity
 *
 * This module defines the Calendar entity, which represents a calendar in the CalDAV
 * implementation. Calendars contain calendar events and are owned by users.
 *
 * Calendars have properties such as name, slug, color, description, supported
 * components, and custom WebDAV/CalDAV dead properties.
 */
use crate::common::errors::{DomainError, ErrorKind, Result};

// Re-export entity errors from the centralized module
pub use super::entity_errors::CalendarError;

pub const DAV_DISPLAYNAME_PROPERTY: &str = "{DAV:}displayname";
pub const DAV_RESOURCETYPE_PROPERTY: &str = "{DAV:}resourcetype";
pub const CALDAV_CALENDAR_DESCRIPTION_PROPERTY: &str =
    "{urn:ietf:params:xml:ns:caldav}calendar-description";
pub const CALDAV_SUPPORTED_COMPONENT_SET_PROPERTY: &str =
    "{urn:ietf:params:xml:ns:caldav}supported-calendar-component-set";
pub const CALDAV_CALENDAR_TIMEZONE_PROPERTY: &str =
    "{urn:ietf:params:xml:ns:caldav}calendar-timezone";
pub const DAV_SYNC_TOKEN_PROPERTY: &str = "{DAV:}sync-token";
pub const CALENDAR_SERVER_GETCTAG_PROPERTY: &str = "{http://calendarserver.org/ns/}getctag";
pub const APPLE_CALENDAR_COLOR_PROPERTY: &str = "{http://apple.com/ns/ical/}calendar-color";
pub const APPLE_CALENDAR_ORDER_PROPERTY: &str = "{http://apple.com/ns/ical/}calendar-order";

const DEFAULT_SUPPORTED_COMPONENT: &str = "VEVENT";
const DEFAULT_CALENDAR_COLOR: &str = "#2C7EF8FF";
const DEFAULT_CALENDAR_ORDER: i32 = 0;
const DEFAULT_CALENDAR_SEQUENCE: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivilegeSet {
    pub read: bool,
    pub write: bool,
}

impl PrivilegeSet {
    pub const fn none() -> Self {
        Self {
            read: false,
            write: false,
        }
    }

    pub const fn read_only() -> Self {
        Self {
            read: true,
            write: false,
        }
    }

    pub const fn read_write() -> Self {
        Self {
            read: true,
            write: true,
        }
    }
}

impl Default for PrivilegeSet {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone)]
pub struct Calendar {
    id: Uuid,
    slug: String,
    name: String,
    owner_id: Uuid,
    description: Option<String>,
    color: Option<String>,
    timezone_text: Option<String>,
    is_public: bool,
    supported_components: Vec<String>,
    ctag: i64,
    sync_token: i64,
    calendar_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    custom_properties: HashMap<String, String>,
}

impl Calendar {
    pub fn new(
        name: String,
        owner_id: Uuid,
        description: Option<String>,
        color: Option<String>,
    ) -> Result<Self> {
        let slug = Self::slug_from_name(&name);
        Self::new_with_slug(
            name,
            slug,
            owner_id,
            description,
            color,
            false,
            None,
            HashMap::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_slug(
        name: String,
        slug: String,
        owner_id: Uuid,
        description: Option<String>,
        color: Option<String>,
        is_public: bool,
        supported_components: Option<Vec<String>>,
        custom_properties: HashMap<String, String>,
    ) -> Result<Self> {
        Self::new_with_slug_and_metadata(
            name,
            slug,
            owner_id,
            description,
            color,
            None,
            is_public,
            supported_components,
            DEFAULT_CALENDAR_SEQUENCE,
            DEFAULT_CALENDAR_SEQUENCE,
            DEFAULT_CALENDAR_ORDER,
            custom_properties,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_slug_and_metadata(
        name: String,
        slug: String,
        owner_id: Uuid,
        description: Option<String>,
        color: Option<String>,
        timezone_text: Option<String>,
        is_public: bool,
        supported_components: Option<Vec<String>>,
        ctag: i64,
        sync_token: i64,
        calendar_order: i32,
        custom_properties: HashMap<String, String>,
    ) -> Result<Self> {
        let now = Utc::now();
        Self::with_id_and_caldav_metadata(
            Uuid::new_v4(),
            name,
            slug,
            owner_id,
            description,
            color,
            timezone_text,
            is_public,
            supported_components.unwrap_or_else(|| vec![DEFAULT_SUPPORTED_COMPONENT.to_string()]),
            ctag,
            sync_token,
            calendar_order,
            now,
            now,
            custom_properties,
        )
    }

    pub fn with_id(
        id: Uuid,
        name: String,
        owner_id: Uuid,
        description: Option<String>,
        color: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self> {
        let slug = Self::slug_from_name(&name);
        Self::with_id_and_details(
            id,
            name,
            slug,
            owner_id,
            description,
            color,
            false,
            vec![DEFAULT_SUPPORTED_COMPONENT.to_string()],
            created_at,
            updated_at,
            HashMap::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_id_and_details(
        id: Uuid,
        name: String,
        slug: String,
        owner_id: Uuid,
        description: Option<String>,
        color: Option<String>,
        is_public: bool,
        supported_components: Vec<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        custom_properties: HashMap<String, String>,
    ) -> Result<Self> {
        let name = Self::normalize_name(name)?;
        let slug = Self::normalize_slug(slug)?;

        if let Some(color_str) = &color {
            Self::validate_color(color_str)?;
        }

        let supported_components = Self::normalize_supported_components(supported_components)?;

        let mut calendar = Self {
            id,
            slug,
            name,
            owner_id,
            description,
            color,
            timezone_text: None,
            is_public,
            supported_components,
            ctag: DEFAULT_CALENDAR_SEQUENCE,
            sync_token: DEFAULT_CALENDAR_SEQUENCE,
            calendar_order: DEFAULT_CALENDAR_ORDER,
            created_at,
            updated_at,
            custom_properties: HashMap::new(),
        };

        calendar.refresh_standard_properties();

        for (property_name, property_value) in custom_properties {
            calendar.set_custom_property_without_touch(property_name, property_value)?;
        }

        Ok(calendar)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_id_and_caldav_metadata(
        id: Uuid,
        name: String,
        slug: String,
        owner_id: Uuid,
        description: Option<String>,
        color: Option<String>,
        timezone_text: Option<String>,
        is_public: bool,
        supported_components: Vec<String>,
        ctag: i64,
        sync_token: i64,
        calendar_order: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        custom_properties: HashMap<String, String>,
    ) -> Result<Self> {
        let name = Self::normalize_name(name)?;
        let slug = Self::normalize_slug(slug)?;
        let color = Some(color.unwrap_or_else(|| DEFAULT_CALENDAR_COLOR.to_string()));
        if let Some(color_str) = &color {
            Self::validate_color(color_str)?;
        }
        let timezone_text = Self::normalize_optional_text(timezone_text);
        let supported_components = Self::normalize_supported_components(supported_components)?;
        Self::validate_positive_sequence("ctag", ctag)?;
        Self::validate_positive_sequence("sync_token", sync_token)?;
        let mut calendar = Self {
            id,
            slug,
            name,
            owner_id,
            description: Self::normalize_optional_text(description),
            color,
            timezone_text,
            is_public,
            supported_components,
            ctag,
            sync_token,
            calendar_order,
            created_at,
            updated_at,
            custom_properties: HashMap::new(),
        };
        calendar.refresh_standard_properties();
        for (property_name, property_value) in custom_properties {
            calendar.set_custom_property_without_touch(property_name, property_value)?;
        }
        Ok(calendar)
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn owner_id(&self) -> &Uuid {
        &self.owner_id
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    pub fn timezone_text(&self) -> Option<&str> {
        self.timezone_text.as_deref()
    }

    pub fn is_public(&self) -> bool {
        self.is_public
    }

    pub fn supported_components(&self) -> &[String] {
        &self.supported_components
    }

    pub fn ctag(&self) -> i64 {
        self.ctag
    }

    pub fn quoted_ctag(&self) -> String {
        format!("\"{}\"", self.ctag)
    }

    pub fn sync_token(&self) -> i64 {
        self.sync_token
    }

    pub fn sync_token_uri(&self, username: &str) -> String {
        format!(
            "http://oxicloud.local/ns/sync/calendars/{}/{}/{}",
            username, self.slug, self.sync_token
        )
    }

    pub fn calendar_order(&self) -> i32 {
        self.calendar_order
    }

    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    pub fn custom_property(&self, name: &str) -> Option<&str> {
        self.custom_properties.get(name).map(|s| s.as_str())
    }

    pub fn custom_properties(&self) -> &HashMap<String, String> {
        &self.custom_properties
    }

    pub fn update_slug(&mut self, slug: String) -> Result<()> {
        self.slug = Self::normalize_slug(slug)?;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn update_name(&mut self, name: String) -> Result<()> {
        self.name = Self::normalize_name(name)?;
        self.refresh_standard_properties();
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn update_description(&mut self, description: Option<String>) {
        self.description = Self::normalize_optional_text(description);
        self.refresh_standard_properties();
        self.updated_at = Utc::now();
    }

    pub fn update_color(&mut self, color: Option<String>) -> Result<()> {
        let color = Some(color.unwrap_or_else(|| DEFAULT_CALENDAR_COLOR.to_string()));
        if let Some(color_str) = &color {
            Self::validate_color(color_str)?;
        }

        self.color = color;
        self.refresh_standard_properties();
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn update_timezone_text(&mut self, timezone_text: Option<String>) {
        self.timezone_text = Self::normalize_optional_text(timezone_text);
        self.refresh_standard_properties();
        self.updated_at = Utc::now();
    }

    pub fn update_caldav_sequences(&mut self, ctag: i64, sync_token: i64) -> Result<()> {
        Self::validate_positive_sequence("ctag", ctag)?;
        Self::validate_positive_sequence("sync_token", sync_token)?;
        self.ctag = ctag;
        self.sync_token = sync_token;
        self.refresh_standard_properties();
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn update_calendar_order(&mut self, calendar_order: i32) {
        self.calendar_order = calendar_order;
        self.refresh_standard_properties();
        self.updated_at = Utc::now();
    }

    pub fn update_public_visibility(&mut self, is_public: bool) {
        self.is_public = is_public;
        self.updated_at = Utc::now();
    }

    pub fn update_supported_components(&mut self, supported_components: Vec<String>) -> Result<()> {
        self.supported_components = Self::normalize_supported_components(supported_components)?;
        self.refresh_standard_properties();
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn set_custom_property(&mut self, name: String, value: String) -> Result<()> {
        self.set_custom_property_without_touch(name, value)?;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn remove_custom_property(&mut self, name: &str) -> bool {
        let result = self.custom_properties.remove(name).is_some();
        if result {
            self.updated_at = Utc::now();
        }
        result
    }

    pub fn belongs_to(&self, user_id: &Uuid) -> bool {
        self.owner_id == *user_id
    }

    pub fn permissions_for(&self, user_id: &Uuid) -> PrivilegeSet {
        if self.owner_id == *user_id {
            PrivilegeSet::read_write()
        } else if self.is_public {
            PrivilegeSet::read_only()
        } else {
            PrivilegeSet::none()
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn slug_from_name(name: &str) -> String {
        let mut slug = String::with_capacity(name.len());
        let mut previous_dash = false;

        for ch in name.trim().chars() {
            let mapped = if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '~') {
                previous_dash = false;
                Some(ch.to_ascii_lowercase())
            } else if ch == '-' || ch.is_whitespace() {
                if previous_dash {
                    None
                } else {
                    previous_dash = true;
                    Some('-')
                }
            } else if previous_dash {
                None
            } else {
                previous_dash = true;
                Some('-')
            };

            if let Some(mapped) = mapped {
                slug.push(mapped);
            }
        }

        let slug = slug.trim_matches('-').to_string();

        if slug.is_empty() {
            format!("calendar-{}", Uuid::new_v4())
        } else {
            slug
        }
    }

    fn normalize_name(name: String) -> Result<String> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar name cannot be empty",
            ));
        }
        Ok(name)
    }

    fn normalize_slug(slug: String) -> Result<String> {
        let slug = slug.trim().trim_matches('/').to_string();

        if slug.is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar slug cannot be empty",
            ));
        }

        if slug == "." || slug == ".." || slug.contains('/') {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar slug must not contain path segments",
            ));
        }

        Ok(slug)
    }

    fn normalize_optional_text(value: Option<String>) -> Option<String> {
        value
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn normalize_supported_components(components: Vec<String>) -> Result<Vec<String>> {
        let mut normalized = Vec::new();

        for component in components {
            let component = component.trim().to_ascii_uppercase();
            if component.is_empty() {
                continue;
            }

            if component
                .chars()
                .any(|ch| !ch.is_ascii_alphanumeric() && ch != '-')
            {
                return Err(DomainError::new(
                    ErrorKind::InvalidInput,
                    "Calendar",
                    "Supported calendar component names must be ASCII alphanumeric values",
                ));
            }

            if !normalized.iter().any(|existing| existing == &component) {
                normalized.push(component);
            }
        }

        if normalized.is_empty() {
            normalized.push(DEFAULT_SUPPORTED_COMPONENT.to_string());
        }

        Ok(normalized)
    }

    fn validate_color(color: &str) -> Result<()> {
        if !color.starts_with('#')
            || !(color.len() == 7 || color.len() == 9)
            || color[1..].chars().any(|c| !c.is_ascii_hexdigit())
        {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Color must be in #RRGGBB or #RRGGBBAA format",
            ));
        }
        Ok(())
    }

    fn validate_positive_sequence(field_name: &str, value: i64) -> Result<()> {
        if value < 1 {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                format!("{field_name} must be greater than zero"),
            ));
        }
        Ok(())
    }

    fn set_custom_property_without_touch(&mut self, name: String, value: String) -> Result<()> {
        if name.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar property name cannot be empty",
            ));
        }

        self.custom_properties.insert(name, value);
        Ok(())
    }

    fn refresh_standard_properties(&mut self) {
        self.custom_properties
            .insert(DAV_DISPLAYNAME_PROPERTY.to_string(), self.name.clone());

        self.custom_properties.insert(
            DAV_RESOURCETYPE_PROPERTY.to_string(),
            "collection,calendar".to_string(),
        );

        self.custom_properties.insert(
            CALDAV_SUPPORTED_COMPONENT_SET_PROPERTY.to_string(),
            self.supported_components.join(","),
        );
        self.custom_properties.insert(
            CALENDAR_SERVER_GETCTAG_PROPERTY.to_string(),
            self.quoted_ctag(),
        );
        self.custom_properties.insert(
            DAV_SYNC_TOKEN_PROPERTY.to_string(),
            self.sync_token.to_string(),
        );
        self.custom_properties.insert(
            APPLE_CALENDAR_ORDER_PROPERTY.to_string(),
            self.calendar_order.to_string(),
        );

        match &self.description {
            Some(description) => {
                self.custom_properties.insert(
                    CALDAV_CALENDAR_DESCRIPTION_PROPERTY.to_string(),
                    description.clone(),
                );
            }
            None => {
                self.custom_properties
                    .remove(CALDAV_CALENDAR_DESCRIPTION_PROPERTY);
            }
        }

        match &self.timezone_text {
            Some(timezone_text) => {
                self.custom_properties.insert(
                    CALDAV_CALENDAR_TIMEZONE_PROPERTY.to_string(),
                    timezone_text.clone(),
                );
            }
            None => {
                self.custom_properties
                    .remove(CALDAV_CALENDAR_TIMEZONE_PROPERTY);
            }
        }

        match &self.color {
            Some(color) => {
                self.custom_properties
                    .insert(APPLE_CALENDAR_COLOR_PROPERTY.to_string(), color.clone());
            }
            None => {
                self.custom_properties.remove(APPLE_CALENDAR_COLOR_PROPERTY);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        let owner_id = Uuid::new_v4();
        let res = Calendar::new("Name".to_string(), owner_id, None, None);
        assert!(res.is_ok());
    }

    #[test]
    fn test_init_with_explicit_slug() {
        let owner_id = Uuid::new_v4();
        let res = Calendar::new_with_slug(
            "Work Calendar".to_string(),
            "work".to_string(),
            owner_id,
            Some("Work schedule".to_string()),
            Some("#84FFa9".to_string()),
            false,
            Some(vec!["VEVENT".to_string()]),
            HashMap::new(),
        );

        assert!(res.is_ok());
        let calendar = res.unwrap();
        assert_eq!(calendar.slug(), "work");
        assert_eq!(calendar.name(), "Work Calendar");
        assert_eq!(calendar.description(), Some("Work schedule"));
        assert_eq!(calendar.color(), Some("#84FFa9"));
        assert_eq!(calendar.supported_components(), &["VEVENT".to_string()]);
        assert_eq!(
            calendar.custom_property(DAV_RESOURCETYPE_PROPERTY),
            Some("collection,calendar")
        );
    }

    #[test]
    fn test_slug_from_name() {
        assert_eq!(Calendar::slug_from_name("Work Calendar"), "work-calendar");
        assert_eq!(
            Calendar::slug_from_name("  Personal.Calendar  "),
            "personal.calendar"
        );
        assert_eq!(Calendar::slug_from_name("A/B C"), "a-b-c");
    }

    #[test]
    fn test_init_color_rgb() {
        let owner_id = Uuid::new_v4();
        let res = Calendar::new(
            "Name".to_string(),
            owner_id,
            None,
            Some("#84FFa9".to_string()),
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_init_color_rgba() {
        let owner_id = Uuid::new_v4();
        let res = Calendar::new(
            "Name".to_string(),
            owner_id,
            None,
            Some("#abcdef51".to_string()),
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_init_bad_color_1() {
        let owner_id = Uuid::new_v4();
        let res = Calendar::new("Name".to_string(), owner_id, None, Some("foo".to_string()));
        assert!(res.is_err());
    }

    #[test]
    fn test_init_bad_color_2() {
        let owner_id = Uuid::new_v4();
        let res = Calendar::new(
            "Name".to_string(),
            owner_id,
            None,
            Some("#xxjjff".to_string()),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_rejects_empty_slug() {
        let owner_id = Uuid::new_v4();
        let res = Calendar::new_with_slug(
            "Name".to_string(),
            " ".to_string(),
            owner_id,
            None,
            None,
            false,
            None,
            HashMap::new(),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_rejects_path_segment_slug() {
        let owner_id = Uuid::new_v4();
        let res = Calendar::new_with_slug(
            "Name".to_string(),
            "../bad".to_string(),
            owner_id,
            None,
            None,
            false,
            None,
            HashMap::new(),
        );
        assert!(res.is_err());
    }
    #[test]
    fn calendar_permissions_owner_gets_read_write() {
        let owner_id = Uuid::new_v4();
        let calendar = Calendar::new("Name".to_string(), owner_id, None, None).unwrap();

        assert_eq!(
            calendar.permissions_for(&owner_id),
            PrivilegeSet::read_write()
        );
    }

    #[test]
    fn calendar_permissions_non_owner_private_gets_none() {
        let owner_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        let calendar = Calendar::new("Name".to_string(), owner_id, None, None).unwrap();

        assert_eq!(
            calendar.permissions_for(&other_user_id),
            PrivilegeSet::none()
        );
    }

    #[test]
    fn calendar_permissions_non_owner_public_gets_read_only() {
        let owner_id = Uuid::new_v4();
        let other_user_id = Uuid::new_v4();
        let calendar = Calendar::new_with_slug(
            "Name".to_string(),
            "name".to_string(),
            owner_id,
            None,
            None,
            true,
            None,
            HashMap::new(),
        )
        .unwrap();

        assert_eq!(
            calendar.permissions_for(&other_user_id),
            PrivilegeSet::read_only()
        );
    }
}
