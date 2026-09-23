mod model;
#[cfg(feature = "server")]
mod provider;

pub use model::{
    GqlProfileSummary, GqlProfileVisibility, ProfileSummary, ProfileSummaryAudience,
    ProfileSummaryVisibility,
};
#[cfg(feature = "server")]
pub use provider::ProfileSummaryReader;

pub type ProfileSummaryReadResult<T> = Result<T, ProfileSummaryReadError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProfileSummaryReadError {
    #[error("profile summary provider is unavailable")]
    Unavailable,
}

impl ProfileSummaryReadError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "profiles.summary_unavailable",
        }
    }
}
