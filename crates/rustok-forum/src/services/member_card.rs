mod member_card {
    use std::collections::{HashMap, HashSet};

    use rustok_profiles::{
        ProfileAccessAudience, ProfilePresentationService, ProfileSummary, ProfilesReader,
    };
    use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    use crate::entities::forum_user_stat;
    use crate::{ForumError, ForumResult};

    pub const MAX_FORUM_MEMBER_CARD_USER_IDS: usize = 100;

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum ForumMemberCardAudience {
        Anonymous,
        Authenticated { actor_id: Uuid },
        TrustedService { actor_id: Option<Uuid> },
    }

    impl ForumMemberCardAudience {
        fn into_profile_audience(self) -> ForumResult<ProfileAccessAudience> {
            match self {
                Self::Anonymous => Ok(ProfileAccessAudience::Anonymous),
                Self::Authenticated { actor_id } => {
                    if actor_id.is_nil() {
                        return Err(ForumError::Validation(
                            "Forum member-card authenticated audience actor must not be nil"
                                .to_string(),
                        ));
                    }
                    Ok(ProfileAccessAudience::Authenticated { actor_id })
                }
                Self::TrustedService { actor_id } => {
                    if actor_id.is_some_and(|actor_id| actor_id.is_nil()) {
                        return Err(ForumError::Validation(
                            "Forum member-card trusted-service audience actor must not be nil"
                                .to_string(),
                        ));
                    }
                    Ok(ProfileAccessAudience::TrustedService { actor_id })
                }
            }
        }
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ForumMemberStats {
        pub topic_count: i32,
        pub reply_count: i32,
        pub solution_count: i32,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ForumMemberCard {
        pub user_id: Uuid,
        pub profile: ProfileSummary,
        pub forum_stats: ForumMemberStats,
    }

    pub struct ForumMemberCardService {
        db: DatabaseConnection,
    }

    impl ForumMemberCardService {
        pub fn new(db: DatabaseConnection) -> Self {
            Self { db }
        }

        pub fn normalize_user_ids(user_ids: &[Uuid]) -> ForumResult<Vec<Uuid>> {
            if user_ids.len() > MAX_FORUM_MEMBER_CARD_USER_IDS {
                return Err(ForumError::Validation(format!(
                    "Forum member-card request exceeds the {MAX_FORUM_MEMBER_CARD_USER_IDS}-user limit"
                )));
            }

            let mut seen = HashSet::with_capacity(user_ids.len());
            let mut normalized = Vec::with_capacity(user_ids.len());
            for user_id in user_ids.iter().copied() {
                if user_id.is_nil() {
                    return Err(ForumError::Validation(
                        "Forum member-card request contains a nil user ID".to_string(),
                    ));
                }
                if seen.insert(user_id) {
                    normalized.push(user_id);
                }
            }
            Ok(normalized)
        }

        /// Compose privacy-admitted profile presentation with Forum-owned statistics.
        ///
        /// Transport authorization remains the caller's responsibility. This method
        /// always applies the Profiles-owned presentation/privacy decision for the
        /// supplied audience before any Forum statistics are queried.
        pub async fn read_for_audience(
            &self,
            tenant_id: Uuid,
            audience: ForumMemberCardAudience,
            user_ids: &[Uuid],
            requested_locale: Option<&str>,
            tenant_default_locale: Option<&str>,
        ) -> ForumResult<Vec<ForumMemberCard>> {
            if tenant_id.is_nil() {
                return Err(ForumError::Validation(
                    "Forum member-card tenant must not be nil".to_string(),
                ));
            }
            let requested_user_ids = Self::normalize_user_ids(user_ids)?;
            if requested_user_ids.is_empty() {
                return Ok(Vec::new());
            }

            let presentation = ProfilePresentationService::for_audience(
                self.db.clone(),
                audience.into_profile_audience()?,
            );
            let profiles = presentation
                .find_profile_summaries(
                    tenant_id,
                    &requested_user_ids,
                    requested_locale,
                    tenant_default_locale,
                )
                .await
                .map_err(profile_presentation_error)?;

            self.compose_admitted_profiles(tenant_id, &requested_user_ids, profiles)
                .await
        }

        pub(crate) async fn compose_admitted_profiles(
            &self,
            tenant_id: Uuid,
            requested_user_ids: &[Uuid],
            mut profiles: HashMap<Uuid, ProfileSummary>,
        ) -> ForumResult<Vec<ForumMemberCard>> {
            if tenant_id.is_nil() {
                return Err(ForumError::Validation(
                    "Forum member-card tenant must not be nil".to_string(),
                ));
            }
            let requested_user_ids = Self::normalize_user_ids(requested_user_ids)?;
            if requested_user_ids.is_empty() || profiles.is_empty() {
                return Ok(Vec::new());
            }

            // Only identities admitted by the Profiles presentation owner may
            // reach the Forum statistics read. Extra profile map entries are ignored.
            let visible_user_ids = requested_user_ids
                .iter()
                .copied()
                .filter(|user_id| profiles.contains_key(user_id))
                .collect::<Vec<_>>();
            if visible_user_ids.is_empty() {
                return Ok(Vec::new());
            }

            let rows = forum_user_stat::Entity::find()
                .filter(forum_user_stat::Column::TenantId.eq(tenant_id))
                .filter(forum_user_stat::Column::UserId.is_in(visible_user_ids.clone()))
                .all(&self.db)
                .await?;
            let mut stats = rows
                .into_iter()
                .map(|row| {
                    (
                        row.user_id,
                        ForumMemberStats {
                            topic_count: row.topic_count,
                            reply_count: row.reply_count,
                            solution_count: row.solution_count,
                        },
                    )
                })
                .collect::<HashMap<_, _>>();

            let mut cards = Vec::with_capacity(visible_user_ids.len());
            for user_id in requested_user_ids {
                let Some(profile) = profiles.remove(&user_id) else {
                    continue;
                };
                cards.push(ForumMemberCard {
                    user_id,
                    profile,
                    forum_stats: stats.remove(&user_id).unwrap_or_default(),
                });
            }
            Ok(cards)
        }
    }

    fn profile_presentation_error(error: rustok_profiles::ProfileError) -> ForumError {
        let source_code = error.code();
        let retryable = error.is_retryable();
        tracing::warn!(
            error = %error,
            source_code,
            retryable,
            "Forum member-card profile presentation failed"
        );
        ForumError::capability_failure(
            "profiles",
            source_code,
            "Profile presentation is unavailable",
            retryable,
        )
    }
}

pub use member_card::{
    ForumMemberCard, ForumMemberCardAudience, ForumMemberCardService, ForumMemberStats,
    MAX_FORUM_MEMBER_CARD_USER_IDS,
};
