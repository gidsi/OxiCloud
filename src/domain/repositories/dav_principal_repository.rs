use uuid::Uuid;

use crate::common::errors::DomainError;
use crate::domain::entities::dav_principal::DavPrincipal;

pub type DavPrincipalRepositoryResult<T> = Result<T, DomainError>;

pub trait DavPrincipalRepository: Send + Sync + 'static {
    async fn upsert_principal(
        &self,
        principal: DavPrincipal,
    ) -> DavPrincipalRepositoryResult<DavPrincipal>;

    async fn get_principal_by_user_id(
        &self,
        user_id: Uuid,
    ) -> DavPrincipalRepositoryResult<DavPrincipal>;

    async fn get_principal_by_principal_path(
        &self,
        principal_path: &str,
    ) -> DavPrincipalRepositoryResult<DavPrincipal>;

    async fn delete_principal(&self, user_id: Uuid) -> DavPrincipalRepositoryResult<()>;
}
