use crate::common::errors::DomainError;
use crate::domain::entities::calendar::Calendar;
use uuid::Uuid;

pub type CalendarRepositoryResult<T> = Result<T, DomainError>;

/// Repository interface for Calendar entity operations
pub trait CalendarRepository: Send + Sync + 'static {
    /// Creates a new calendar
    async fn create_calendar(&self, calendar: Calendar) -> CalendarRepositoryResult<Calendar>;

    /// Updates an existing calendar
    async fn update_calendar(&self, calendar: Calendar) -> CalendarRepositoryResult<Calendar>;

    /// Deletes a calendar by ID
    async fn delete_calendar(&self, id: &Uuid) -> CalendarRepositoryResult<()>;

    /// Finds a calendar by its ID
    async fn find_calendar_by_id(&self, id: &Uuid) -> CalendarRepositoryResult<Calendar>;


    /// Finds a calendar by stable CalDAV collection slug and owner
    async fn find_calendar_by_slug_and_owner(
        &self,
        slug: &str,
        owner_id: Uuid,
    ) -> CalendarRepositoryResult<Calendar>;

    /// Returns true when a calendar collection already exists at the owner/slug URI
    async fn calendar_exists_by_slug_and_owner(
        &self,
        slug: &str,
        owner_id: Uuid,
    ) -> CalendarRepositoryResult<bool>;

    /// Lists all calendars for a specific user
    async fn list_calendars_by_owner(
        &self,
        owner_id: Uuid,
    ) -> CalendarRepositoryResult<Vec<Calendar>>;

    /// Finds a calendar by name and owner
    async fn find_calendar_by_name_and_owner(
        &self,
        name: &str,
        owner_id: Uuid,
    ) -> CalendarRepositoryResult<Calendar>;

    /// Lists calendars shared with a specific user
    async fn list_calendars_shared_with_user(
        &self,
        user_id: Uuid,
    ) -> CalendarRepositoryResult<Vec<Calendar>>;

    /// List public calendars
    async fn list_public_calendars(
        &self,
        limit: i64,
        offset: i64,
    ) -> CalendarRepositoryResult<Vec<Calendar>>;

    /// Checks if a user has access to a calendar
    async fn user_has_calendar_access(
        &self,
        calendar_id: &Uuid,
        user_id: Uuid,
    ) -> CalendarRepositoryResult<bool>;

    /// Gets a custom property for a calendar
    async fn get_calendar_property(
        &self,
        calendar_id: &Uuid,
        property_name: &str,
    ) -> CalendarRepositoryResult<Option<String>>;

    /// Sets a custom property for a calendar
    async fn set_calendar_property(
        &self,
        calendar_id: &Uuid,
        property_name: &str,
        property_value: &str,
    ) -> CalendarRepositoryResult<()>;

    /// Sets multiple custom properties for a calendar
    async fn set_calendar_properties(
        &self,
        calendar_id: &Uuid,
        properties: &std::collections::HashMap<String, String>,
    ) -> CalendarRepositoryResult<()> {
        for (property_name, property_value) in properties {
            self.set_calendar_property(calendar_id, property_name, property_value)
                .await?;
        }

        Ok(())
    }

    /// Removes a custom property from a calendar
    async fn remove_calendar_property(
        &self,
        calendar_id: &Uuid,
        property_name: &str,
    ) -> CalendarRepositoryResult<()>;

    /// Gets all custom properties for a calendar
    async fn get_calendar_properties(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarRepositoryResult<std::collections::HashMap<String, String>>;

    /// Share calendar with another user
    async fn share_calendar(
        &self,
        calendar_id: &Uuid,
        user_id: Uuid,
        access_level: &str,
    ) -> CalendarRepositoryResult<()>;

    /// Remove calendar sharing for a user
    async fn remove_calendar_sharing(
        &self,
        calendar_id: &Uuid,
        user_id: Uuid,
    ) -> CalendarRepositoryResult<()>;

    /// Get calendar sharing information (who has access to this calendar)
    async fn get_calendar_shares(
        &self,
        calendar_id: &Uuid,
    ) -> CalendarRepositoryResult<Vec<(String, String)>>;
}
