mod model;
mod provider;

pub use model::{
    GqlProfileSummary, GqlProfileVisibility, ProfileSummary, ProfileSummaryAudience,
    ProfileSummaryVisibility,
};
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_profile_summary_audience_properties() {
        let anon = ProfileSummaryAudience::Anonymous;
        assert!(!anon.is_authenticated());
        assert_eq!(anon.actor_id(), None);

        let uid = Uuid::new_v4();
        let auth = ProfileSummaryAudience::Authenticated { actor_id: uid };
        assert!(auth.is_authenticated());
        assert_eq!(auth.actor_id(), Some(uid));

        let trusted = ProfileSummaryAudience::TrustedService {
            actor_id: Some(uid),
        };
        assert!(trusted.is_authenticated());
        assert_eq!(trusted.actor_id(), Some(uid));

        let trusted_none = ProfileSummaryAudience::TrustedService { actor_id: None };
        assert!(trusted_none.is_authenticated());
        assert_eq!(trusted_none.actor_id(), None);
    }

    #[test]
    fn test_visibility_conversions() {
        assert_eq!(
            GqlProfileVisibility::from(ProfileSummaryVisibility::Public),
            GqlProfileVisibility::Public
        );
        assert_eq!(
            GqlProfileVisibility::from(ProfileSummaryVisibility::Authenticated),
            GqlProfileVisibility::Authenticated
        );
        assert_eq!(
            GqlProfileVisibility::from(ProfileSummaryVisibility::FollowersOnly),
            GqlProfileVisibility::FollowersOnly
        );
        assert_eq!(
            GqlProfileVisibility::from(ProfileSummaryVisibility::Private),
            GqlProfileVisibility::Private
        );
    }

    #[test]
    fn test_error_code() {
        let err = ProfileSummaryReadError::Unavailable;
        assert_eq!(err.code(), "profiles.summary_unavailable");
        assert_eq!(err.to_string(), "profile summary provider is unavailable");
    }
}
