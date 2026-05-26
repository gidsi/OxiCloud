use chrono::{DateTime, Utc};
/**
 * Calendar Entity
 *
 * This module defines the Calendar entity, which represents a calendar in the CalDAV
 * implementation. Calendars contain calendar events and are owned by users.
 *
 * Calendars have properties such as name, color, and description, and they serve as
 * containers for calendar events. Each calendar belongs to a specific user and can
 * have custom properties.
 */
use uuid::Uuid;

use crate::common::errors::{DomainError, ErrorKind, Result};

// Re-export entity errors from the centralized module
pub use super::entity_errors::CalendarError;

/**
 * Calendar entity.
 *
 * Represents a calendar container that can hold multiple calendar events.
 * Each calendar is owned by a user and has properties like name, color, and description.
 */
#[derive(Debug, Clone)]
pub struct Calendar {
    /// Unique identifier for the calendar
    id: Uuid,

    /// Stable collection name used in URLs
    name: String,

    /// Human-readable display name exposed through DAV `displayname`.
    display_name: String,

    /// ID of the user who owns this calendar
    owner_id: Uuid,

    /// Optional description of the calendar
    description: Option<String>,

    /// Optional color code for UI display (hex format #RRGGBB or #RRGGBBAA)
    color: Option<String>,

    /// Whether this calendar is publicly visible.
    is_public: bool,

    /// Calendar collection change tag used by CalDAV clients.
    ctag: String,

    /// Monotonic sync version used for DAV sync-token generation.
    sync_version: i64,

    /// Supported iCalendar component types for this collection.
    supported_components: Vec<String>,

    /// Optional calendar timezone payload.
    timezone: Option<String>,

    /// Client-visible ordering hint.
    calendar_order: i32,

    /// Time when the calendar was created
    created_at: DateTime<Utc>,

    /// Time when the calendar was last modified
    updated_at: DateTime<Utc>,

    /// Optional list of custom properties (for extended CalDAV support)
    custom_properties: std::collections::HashMap<String, String>,
}

impl Calendar {
    /**
     * Creates a new calendar with the given properties.
     *
     * @param name Display name of the calendar
     * @param owner_id ID of the user who owns this calendar
     * @param description Optional description of the calendar
     * @param color Optional color code for UI display (#RRGGBB format)
     * @return Result containing the new Calendar or a domain error
     */
    pub fn new(
        name: String,
        owner_id: Uuid,
        description: Option<String>,
        color: Option<String>,
    ) -> Result<Self> {
        Self::new_with_display_name(
            name.clone(),
            name,
            owner_id,
            description,
            color,
            false,
            vec!["VEVENT".to_string(), "VTODO".to_string()],
            None,
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_display_name(
        name: String,
        display_name: String,
        owner_id: Uuid,
        description: Option<String>,
        color: Option<String>,
        is_public: bool,
        supported_components: Vec<String>,
        timezone: Option<String>,
        calendar_order: i32,
    ) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar name cannot be empty",
            ));
        }

        if display_name.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar display name cannot be empty",
            ));
        }

        if let Some(color_str) = &color {
            Self::validate_color(color_str)?;
        }

        Self::validate_supported_components(&supported_components)?;

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4(),
            name,
            display_name,
            owner_id,
            description,
            color,
            is_public,
            ctag: "1".to_string(),
            sync_version: 1,
            supported_components,
            timezone,
            calendar_order,
            created_at: now,
            updated_at: now,
            custom_properties: std::collections::HashMap::new(),
        })
    }

    /**
     * Creates a calendar with specific ID and timestamps.
     * Typically used when reconstructing from storage.
     *
     * @param id Unique identifier for the calendar
     * @param name Display name of the calendar
     * @param owner_id ID of the user who owns this calendar
     * @param description Optional description of the calendar
     * @param color Optional color code for UI display
     * @param created_at Time when the calendar was created
     * @param updated_at Time when the calendar was last modified
     * @return Result containing the new Calendar or a domain error
     */
    pub fn with_id(
        id: Uuid,
        name: String,
        owner_id: Uuid,
        description: Option<String>,
        color: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self> {
        Self::with_dav_metadata(
            id,
            name.clone(),
            name,
            owner_id,
            description,
            color,
            false,
            "1".to_string(),
            1,
            vec!["VEVENT".to_string(), "VTODO".to_string()],
            None,
            0,
            created_at,
            updated_at,
        )
    }

    /// Creates a calendar with all DAV persistence metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn with_dav_metadata(
        id: Uuid,
        name: String,
        display_name: String,
        owner_id: Uuid,
        description: Option<String>,
        color: Option<String>,
        is_public: bool,
        ctag: String,
        sync_version: i64,
        supported_components: Vec<String>,
        timezone: Option<String>,
        calendar_order: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self> {
        if name.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar name cannot be empty",
            ));
        }
        if display_name.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar display name cannot be empty",
            ));
        }
        if let Some(color_str) = &color {
            Self::validate_color(color_str)?;
        }
        Self::validate_sync_version(sync_version)?;
        Self::validate_supported_components(&supported_components)?;
        Ok(Self {
            id,
            name,
            display_name,
            owner_id,
            description,
            color,
            is_public,
            ctag,
            sync_version,
            supported_components,
            timezone,
            calendar_order,
            created_at,
            updated_at,
            custom_properties: std::collections::HashMap::new(),
        })
    }

    // Getters

    /// Returns the calendar's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Returns the calendar's display name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the DAV display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the ID of the user who owns this calendar
    pub fn owner_id(&self) -> &Uuid {
        &self.owner_id
    }

    /// Returns the calendar's description, if any
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Returns the calendar's color code, if any
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    pub fn is_public(&self) -> bool {
        self.is_public
    }
    pub fn ctag(&self) -> &str {
        &self.ctag
    }
    pub fn sync_version(&self) -> i64 {
        self.sync_version
    }
    pub fn supported_components(&self) -> &[String] {
        &self.supported_components
    }
    pub fn timezone(&self) -> Option<&str> {
        self.timezone.as_deref()
    }
    pub fn calendar_order(&self) -> i32 {
        self.calendar_order
    }

    /// Returns the time when the calendar was created
    pub fn created_at(&self) -> &DateTime<Utc> {
        &self.created_at
    }

    /// Returns the time when the calendar was last modified
    pub fn updated_at(&self) -> &DateTime<Utc> {
        &self.updated_at
    }

    /// Returns a custom property value by name, if it exists
    pub fn custom_property(&self, name: &str) -> Option<&str> {
        self.custom_properties.get(name).map(|s| s.as_str())
    }

    /// Returns all custom properties
    pub fn custom_properties(&self) -> &std::collections::HashMap<String, String> {
        &self.custom_properties
    }

    // Setters and Mutators

    /**
     * Updates the calendar's name.
     *
     * @param name New display name for the calendar
     * @return Result indicating success or containing a domain error
     */
    pub fn update_name(&mut self, name: String) -> Result<()> {
        if name.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar name cannot be empty",
            ));
        }

        self.name = name;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn update_display_name(&mut self, display_name: String) -> Result<()> {
        if display_name.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar display name cannot be empty",
            ));
        }
        self.display_name = display_name;
        self.updated_at = Utc::now();
        Ok(())
    }

    /**
     * Updates the calendar's description.
     *
     * @param description New description for the calendar
     */
    pub fn update_description(&mut self, description: Option<String>) {
        self.description = description;
        self.updated_at = Utc::now();
    }

    /**
     * Updates the calendar's color.
     *
     * @param color New color code for the calendar
     * @return Result indicating success or containing a domain error
     */
    pub fn update_color(&mut self, color: Option<String>) -> Result<()> {
        // Validate color format if provided
        if let Some(color_str) = &color {
            Self::validate_color(color_str)?;
        }

        self.color = color;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn update_is_public(&mut self, is_public: bool) {
        self.is_public = is_public;
        self.updated_at = Utc::now();
    }
    pub fn update_sync_metadata(&mut self, ctag: String, sync_version: i64) -> Result<()> {
        Self::validate_sync_version(sync_version)?;
        self.ctag = ctag;
        self.sync_version = sync_version;
        self.updated_at = Utc::now();
        Ok(())
    }
    pub fn update_supported_components(&mut self, supported_components: Vec<String>) -> Result<()> {
        Self::validate_supported_components(&supported_components)?;
        self.supported_components = supported_components;
        self.updated_at = Utc::now();
        Ok(())
    }
    pub fn update_timezone(&mut self, timezone: Option<String>) {
        self.timezone = timezone;
        self.updated_at = Utc::now();
    }
    pub fn update_calendar_order(&mut self, calendar_order: i32) {
        self.calendar_order = calendar_order;
        self.updated_at = Utc::now();
    }

    /// Validate a calendar color
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

    fn validate_sync_version(sync_version: i64) -> Result<()> {
        if sync_version < 1 {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar sync version must be positive",
            ));
        }
        Ok(())
    }

    fn validate_supported_components(supported_components: &[String]) -> Result<()> {
        if supported_components.is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "Calendar",
                "Calendar must support at least one component type",
            ));
        }
        for component in supported_components {
            if component != "VEVENT" && component != "VTODO" {
                return Err(DomainError::new(
                    ErrorKind::InvalidInput,
                    "Calendar",
                    "Supported calendar components are VEVENT and VTODO",
                ));
            }
        }
        Ok(())
    }

    /**
     * Sets a custom property for extended CalDAV support.
     *
     * @param name Name of the property
     * @param value Value of the property
     */
    pub fn set_custom_property(&mut self, name: String, value: String) {
        self.custom_properties.insert(name, value);
        self.updated_at = Utc::now();
    }

    /**
     * Removes a custom property.
     *
     * @param name Name of the property to remove
     * @return true if the property was removed, false if it didn't exist
     */
    pub fn remove_custom_property(&mut self, name: &str) -> bool {
        let result = self.custom_properties.remove(name).is_some();
        if result {
            self.updated_at = Utc::now();
        }
        result
    }

    /**
     * Checks if this calendar belongs to the specified user.
     *
     * @param user_id ID of the user to check ownership against
     * @return true if the calendar belongs to the user, false otherwise
     */
    pub fn belongs_to(&self, user_id: &Uuid) -> bool {
        self.owner_id == *user_id
    }

    /**
     * Updates the last modification time of the calendar to now.
     * Called when calendar events are added, modified, or removed.
     */
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
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

    /// Format as used by the android DAVx app
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
}
