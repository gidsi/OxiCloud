use uuid::Uuid;

use crate::application::dtos::dav_principal_dto::{DavPrincipalDto, DavPrincipalHomeSetsDto};
use crate::common::errors::DomainError;

pub trait DavPrincipalStoragePort: Send + Sync + 'static {
    async fn get_principal_by_user_id(&self, user_id: Uuid)
    -> Result<DavPrincipalDto, DomainError>;

    async fn get_principal_by_principal_path(
        &self,
        principal_path: &str,
    ) -> Result<DavPrincipalDto, DomainError>;

    async fn upsert_principal_for_user(
        &self,
        user_id: Uuid,
        username: &str,
    ) -> Result<DavPrincipalDto, DomainError>;
}

pub trait DavPrincipalDiscoveryUseCase: Send + Sync + 'static {
    async fn get_principal_home_sets(
        &self,
        user_id: Uuid,
    ) -> Result<DavPrincipalHomeSetsDto, DomainError>;

    async fn get_principal_home_sets_by_path(
        &self,
        principal_path: &str,
        authenticated_user_id: Uuid,
    ) -> Result<DavPrincipalHomeSetsDto, DomainError>;
}
