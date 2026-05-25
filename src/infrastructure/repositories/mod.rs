// Repositorios PostgreSQL (blob-storage model)
pub mod pg;

// Re-exportar para facilitar acceso
pub use pg::{
    AppPasswordPgRepository, DavPrincipalPgRepository, DeviceCodePgRepository,
    FileBlobReadRepository, FileBlobWriteRepository, FolderDbRepository, SessionPgRepository,
    TrashDbRepository, UserPgRepository,
};
