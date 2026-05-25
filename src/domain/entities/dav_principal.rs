use chrono::{DateTime, Utc};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use uuid::Uuid;

use crate::common::errors::{DomainError, ErrorKind, Result};

const DAV_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b'\\');

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DavPrincipal {
    user_id: Uuid,
    username: String,
    principal_path: String,
    calendar_home_set_path: String,
    addressbook_home_set_path: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl DavPrincipal {
    pub fn new(user_id: Uuid, username: String) -> Result<Self> {
        Self::validate_username(&username)?;

        let encoded_username = utf8_percent_encode(&username, DAV_SEGMENT_ENCODE_SET).to_string();
        let now = Utc::now();

        Self::from_data(
            user_id,
            username,
            format!("/caldav/principals/{encoded_username}/"),
            format!("/caldav/{encoded_username}/"),
            format!("/carddav/{encoded_username}/"),
            now,
            now,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_data(
        user_id: Uuid,
        username: String,
        principal_path: String,
        calendar_home_set_path: String,
        addressbook_home_set_path: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self> {
        Self::validate_username(&username)?;
        Self::validate_principal_path(&principal_path)?;
        Self::validate_home_set_path(&calendar_home_set_path, "/caldav/")?;
        Self::validate_home_set_path(&addressbook_home_set_path, "/carddav/")?;

        Ok(Self {
            user_id,
            username,
            principal_path,
            calendar_home_set_path,
            addressbook_home_set_path,
            created_at,
            updated_at,
        })
    }

    pub fn user_id(&self) -> Uuid {
        self.user_id
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn principal_path(&self) -> &str {
        &self.principal_path
    }

    pub fn calendar_home_set_path(&self) -> &str {
        &self.calendar_home_set_path
    }

    pub fn addressbook_home_set_path(&self) -> &str {
        &self.addressbook_home_set_path
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    fn validate_username(username: &str) -> Result<()> {
        if username.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "DavPrincipal",
                "Username cannot be empty",
            ));
        }

        if username.contains('/') || username.contains('\\') {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "DavPrincipal",
                "Username cannot contain path separators",
            ));
        }

        Ok(())
    }

    fn validate_principal_path(path: &str) -> Result<()> {
        if !path.starts_with("/caldav/principals/") || !path.ends_with('/') {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "DavPrincipal",
                "Principal path must start with /caldav/principals/ and end with /",
            ));
        }

        Self::validate_absolute_collection_path(path)
    }

    fn validate_home_set_path(path: &str, required_prefix: &str) -> Result<()> {
        if !path.starts_with(required_prefix) || !path.ends_with('/') {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "DavPrincipal",
                format!("Home-set path must start with {required_prefix} and end with /"),
            ));
        }

        Self::validate_absolute_collection_path(path)
    }

    fn validate_absolute_collection_path(path: &str) -> Result<()> {
        if path.is_empty() || !path.starts_with('/') || !path.ends_with('/') {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "DavPrincipal",
                "DAV paths must be absolute collection paths",
            ));
        }

        if path[1..].contains("//") || path.contains("/../") || path.contains("/./") {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "DavPrincipal",
                "DAV paths cannot contain traversal or duplicate path separators",
            ));
        }

        Ok(())
    }
}
