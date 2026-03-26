use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

pub type ChannelResult<T> = Result<T, ChannelError>;

#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("channel `{0}` already exists for this tenant")]
    SlugAlreadyExists(String),
    #[error("channel {0} not found")]
    NotFound(Uuid),
    #[error("target type `{0}` is invalid")]
    InvalidTargetType(String),
    #[error(transparent)]
    Database(#[from] DbErr),
}
