use sea_orm::DbErr;
use thiserror::Error;

pub type SocialGraphResult<T> = Result<T, SocialGraphError>;

#[derive(Debug, Error)]
pub enum SocialGraphError {
    #[error("social graph tenant identifier is invalid")]
    InvalidTenantId,
    #[error("social graph relation cannot target the source user")]
    SelfRelation,
    #[error("social graph relation revision changed before the command was applied")]
    RevisionConflict,
    #[error("social graph command actor does not own the relation source")]
    SourceActorMismatch,
    #[error(transparent)]
    Database(#[from] DbErr),
}
