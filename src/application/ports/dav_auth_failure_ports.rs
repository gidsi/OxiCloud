use chrono::{DateTime, Utc};

use crate::application::dtos::dav_auth_failure_dto::{CreateDavAuthFailureDto, DavAuthFailureDto};
use crate::common::errors::DomainError;

/// Application storage port for DAV authentication failure auditing.
pub trait DavAuthFailureStoragePort: Send + Sync + 'static {
    /// Persist a failed DAV authentication attempt.
    async fn record_failure(
        &self,
        failure: CreateDavAuthFailureDto,
    ) -> Result<DavAuthFailureDto, DomainError>;

    /// Count recent failures for an IP address.
    async fn count_failures_by_client_ip_since(
        &self,
        client_ip: &str,
        since: DateTime<Utc>,
    ) -> Result<i64, DomainError>;

    /// List recent failures for diagnostics.
    async fn list_failures_by_client_ip(
        &self,
        client_ip: &str,
        limit: i64,
    ) -> Result<Vec<DavAuthFailureDto>, DomainError>;
}
