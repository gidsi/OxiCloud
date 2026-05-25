use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::errors::DomainError;
use crate::domain::entities::dav_auth_failure::DavAuthFailure;

/// DTO used by middleware/application services to record a failed DAV auth
/// attempt without exposing persistence details.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateDavAuthFailureDto {
    #[serde(default)]
    pub client_ip: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub user_agent: String,
    pub reason: String,
    #[serde(default = "default_auth_scheme")]
    pub auth_scheme: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_auth_scheme() -> String {
    "Basic".to_string()
}

fn default_protocol() -> String {
    "DAV".to_string()
}

impl TryFrom<CreateDavAuthFailureDto> for DavAuthFailure {
    type Error = DomainError;

    fn try_from(dto: CreateDavAuthFailureDto) -> Result<Self, Self::Error> {
        DavAuthFailure::from_data(
            Uuid::new_v4(),
            Utc::now(),
            dto.client_ip,
            dto.username,
            dto.method,
            dto.path,
            dto.user_agent,
            dto.reason,
            dto.auth_scheme,
            dto.protocol,
        )
    }
}

/// Persisted DAV authentication failure audit record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DavAuthFailureDto {
    pub id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub client_ip: String,
    pub username: String,
    pub method: String,
    pub path: String,
    pub user_agent: String,
    pub reason: String,
    pub auth_scheme: String,
    pub protocol: String,
}

impl From<DavAuthFailure> for DavAuthFailureDto {
    fn from(failure: DavAuthFailure) -> Self {
        Self {
            id: failure.id(),
            occurred_at: failure.occurred_at(),
            client_ip: failure.client_ip().to_string(),
            username: failure.username().to_string(),
            method: failure.method().to_string(),
            path: failure.path().to_string(),
            user_agent: failure.user_agent().to_string(),
            reason: failure.reason_code().to_string(),
            auth_scheme: failure.auth_scheme().to_string(),
            protocol: failure.protocol().to_string(),
        }
    }
}
