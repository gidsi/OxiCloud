//! DAV authentication failure audit entity.
//!
//! Represents a failed Basic Authentication attempt against protected DAV
//! endpoints. These records are persisted for operator visibility and can be
//! consumed by external tooling such as Fail2Ban.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::common::errors::{DomainError, ErrorKind, Result};

/// Machine-readable DAV authentication failure reason.
///
/// The string values are intentionally stable because they are persisted in
/// `auth.dav_auth_failures.reason` and may be parsed by external tooling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DavAuthFailureReason {
    MissingCredentials,
    MalformedCredentials,
    InvalidCredentials,
    ExpiredCredentials,
    RevokedCredentials,
    UnsupportedScheme,
    AppPasswordsDisabled,
    Other(String),
}

impl DavAuthFailureReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::MissingCredentials => "missing_credentials",
            Self::MalformedCredentials => "malformed_credentials",
            Self::InvalidCredentials => "invalid_credentials",
            Self::ExpiredCredentials => "expired_credentials",
            Self::RevokedCredentials => "revoked_credentials",
            Self::UnsupportedScheme => "unsupported_scheme",
            Self::AppPasswordsDisabled => "app_passwords_disabled",
            Self::Other(reason) => reason.as_str(),
        }
    }

    pub fn parse(reason: &str) -> Result<Self> {
        let trimmed = reason.trim();

        if trimmed.is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "DavAuthFailure",
                "Failure reason cannot be empty",
            ));
        }

        Ok(match trimmed {
            "missing_credentials" => Self::MissingCredentials,
            "malformed_credentials" => Self::MalformedCredentials,
            "invalid_credentials" => Self::InvalidCredentials,
            "expired_credentials" => Self::ExpiredCredentials,
            "revoked_credentials" => Self::RevokedCredentials,
            "unsupported_scheme" => Self::UnsupportedScheme,
            "app_passwords_disabled" => Self::AppPasswordsDisabled,
            other => Self::Other(other.to_string()),
        })
    }
}

impl std::fmt::Display for DavAuthFailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Durable audit record for a failed DAV Basic Auth attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DavAuthFailure {
    id: Uuid,
    occurred_at: DateTime<Utc>,
    client_ip: String,
    username: String,
    method: String,
    path: String,
    user_agent: String,
    reason: DavAuthFailureReason,
    auth_scheme: String,
    protocol: String,
}

impl DavAuthFailure {
    /// Create a new DAV Basic Auth failure audit record.
    pub fn new(
        client_ip: String,
        username: String,
        method: String,
        path: String,
        user_agent: String,
        reason: DavAuthFailureReason,
    ) -> Result<Self> {
        Self::from_data(
            Uuid::new_v4(),
            Utc::now(),
            client_ip,
            username,
            method,
            path,
            user_agent,
            reason.as_str().to_string(),
            "Basic".to_string(),
            "DAV".to_string(),
        )
    }

    /// Reconstruct a DAV auth failure from persistence.
    #[allow(clippy::too_many_arguments)]
    pub fn from_data(
        id: Uuid,
        occurred_at: DateTime<Utc>,
        client_ip: String,
        username: String,
        method: String,
        path: String,
        user_agent: String,
        reason: String,
        auth_scheme: String,
        protocol: String,
    ) -> Result<Self> {
        Self::validate_non_empty(&auth_scheme, "Authentication scheme")?;
        Self::validate_non_empty(&protocol, "Protocol")?;

        if !path.is_empty() && !path.starts_with('/') {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "DavAuthFailure",
                "DAV request path must be empty or an absolute path",
            ));
        }

        Ok(Self {
            id,
            occurred_at,
            client_ip,
            username,
            method,
            path,
            user_agent,
            reason: DavAuthFailureReason::parse(&reason)?,
            auth_scheme,
            protocol,
        })
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    pub fn client_ip(&self) -> &str {
        &self.client_ip
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    pub fn reason(&self) -> &DavAuthFailureReason {
        &self.reason
    }

    pub fn reason_code(&self) -> &str {
        self.reason.as_str()
    }

    pub fn auth_scheme(&self) -> &str {
        &self.auth_scheme
    }

    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    fn validate_non_empty(value: &str, field_name: &str) -> Result<()> {
        if value.trim().is_empty() {
            return Err(DomainError::new(
                ErrorKind::InvalidInput,
                "DavAuthFailure",
                format!("{field_name} cannot be empty"),
            ));
        }

        Ok(())
    }
}
