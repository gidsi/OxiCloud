use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::entities::dav_principal::DavPrincipal;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DavPrincipalDto {
    pub user_id: Uuid,
    pub username: String,
    pub principal_path: String,
    pub calendar_home_set_path: String,
    pub addressbook_home_set_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DavPrincipal> for DavPrincipalDto {
    fn from(principal: DavPrincipal) -> Self {
        Self {
            user_id: principal.user_id(),
            username: principal.username().to_string(),
            principal_path: principal.principal_path().to_string(),
            calendar_home_set_path: principal.calendar_home_set_path().to_string(),
            addressbook_home_set_path: principal.addressbook_home_set_path().to_string(),
            created_at: principal.created_at(),
            updated_at: principal.updated_at(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DavPrincipalHomeSetsDto {
    pub user_id: Uuid,
    pub username: String,
    pub principal_path: String,
    pub calendar_home_set_path: String,
    pub addressbook_home_set_path: String,
}

impl From<DavPrincipalDto> for DavPrincipalHomeSetsDto {
    fn from(principal: DavPrincipalDto) -> Self {
        Self {
            user_id: principal.user_id,
            username: principal.username,
            principal_path: principal.principal_path,
            calendar_home_set_path: principal.calendar_home_set_path,
            addressbook_home_set_path: principal.addressbook_home_set_path,
        }
    }
}

impl From<DavPrincipal> for DavPrincipalHomeSetsDto {
    fn from(principal: DavPrincipal) -> Self {
        Self {
            user_id: principal.user_id(),
            username: principal.username().to_string(),
            principal_path: principal.principal_path().to_string(),
            calendar_home_set_path: principal.calendar_home_set_path().to_string(),
            addressbook_home_set_path: principal.addressbook_home_set_path().to_string(),
        }
    }
}
