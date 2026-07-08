use sea_orm::DatabaseConnection;
use thiserror::Error;

pub use rustok_api::HostRuntimeContext;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RuntimeHandleError {
    #[error("required host runtime handle is missing: {handle}")]
    MissingSharedHandle { handle: &'static str },
}

pub type RuntimeHandleResult<T> = Result<T, RuntimeHandleError>;

pub fn db_clone(runtime: &HostRuntimeContext) -> DatabaseConnection {
    runtime.db_clone()
}

pub fn require_shared<T>(
    runtime: &HostRuntimeContext,
    handle: &'static str,
) -> RuntimeHandleResult<T>
where
    T: 'static + Send + Sync + Clone,
{
    runtime
        .shared_get::<T>()
        .ok_or(RuntimeHandleError::MissingSharedHandle { handle })
}
