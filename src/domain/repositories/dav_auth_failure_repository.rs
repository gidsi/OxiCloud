use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::common::errors::DomainError;
use crate::domain::entities::dav_auth_failure::DavAuthFailure;

pub type DavAuthFailureRepositoryResult<T> = Result<T, DomainError>;

/// Repository for persisted DAV authentication failure audit records.
pub trait DavAuthFailureRepository: Send + Sync + 'static {
    /// Persist a failed DAV authentication attempt.
    async fn create_failure(
        &self,
        failure: DavAuthFailure,
    ) -> DavAuthFailureRepositoryResult<DavAuthFailure>;

    /// Fetch a specific DAV authentication failure audit record.
    async fn get_failure_by_id(&self, id: Uuid) -> DavAuthFailureRepositoryResult<DavAuthFailure>;

    /// Count recent failures for an IP address.
    async fn count_failures_by_client_ip_since(
        &self,
        client_ip: &str,
        since: DateTime<Utc>,
    ) -> DavAuthFailureRepositoryResult<i64>;

    /// List recent failures for an IP address for operator diagnostics.
    async fn list_failures_by_client_ip(
        &self,
        client_ip: &str,
        limit: i64,
    ) -> DavAuthFailureRepositoryResult<Vec<DavAuthFailure>>;
}
