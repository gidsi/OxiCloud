use std::sync::Arc;

use uuid::Uuid;

use crate::application::dtos::dav_principal_dto::DavPrincipalHomeSetsDto;
use crate::application::ports::dav_principal_ports::{
    DavPrincipalDiscoveryUseCase, DavPrincipalStoragePort,
};
use crate::common::errors::{DomainError, ErrorKind};
use crate::infrastructure::repositories::pg::DavPrincipalPgRepository;

pub struct DavPrincipalService {
    storage: Arc<DavPrincipalPgRepository>,
}

impl DavPrincipalService {
    pub fn new(storage: Arc<DavPrincipalPgRepository>) -> Self {
        Self { storage }
    }
}

impl DavPrincipalDiscoveryUseCase for DavPrincipalService {
    async fn get_principal_home_sets(
        &self,
        user_id: Uuid,
    ) -> Result<DavPrincipalHomeSetsDto, DomainError> {
        Ok(self.storage.get_principal_by_user_id(user_id).await?.into())
    }

    async fn get_principal_home_sets_by_path(
        &self,
        principal_path: &str,
        authenticated_user_id: Uuid,
    ) -> Result<DavPrincipalHomeSetsDto, DomainError> {
        let principal = self
            .storage
            .get_principal_by_principal_path(principal_path)
            .await?;

        if principal.user_id != authenticated_user_id {
            return Err(DomainError::new(
                ErrorKind::AccessDenied,
                "DavPrincipal",
                "Authenticated user cannot access the requested DAV principal",
            ));
        }

        Ok(principal.into())
    }
}
