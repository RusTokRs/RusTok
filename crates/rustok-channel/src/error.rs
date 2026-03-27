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
    #[error("channel {0} is not active")]
    InactiveChannel(Uuid),
    #[error("target type `{0}` is invalid")]
    InvalidTargetType(String),
    #[error("target value `{0}` is invalid")]
    InvalidTargetValue(String),
    #[error("target `{1}` already exists for target type `{0}` in this tenant")]
    TargetAlreadyExists(String, String),
    #[error(transparent)]
    Database(#[from] DbErr),
}
