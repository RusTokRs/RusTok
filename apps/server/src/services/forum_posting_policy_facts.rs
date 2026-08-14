use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_forum::{
    ForumApprovedPostsFactPort, ForumPostingPolicyFactKind, ForumPostingPolicyFactsComposer,
    ForumPostingPolicyOwnerFactPort, ForumPostingPolicyOwnerFactRequest,
    ForumPostingPolicyOwnerFactResponse, ForumPostingPolicyOwnerFactValue,
    ForumPostingTrustFactPort, ForumReplyCreatesWindowFactPort, ForumTopicCreatesWindowFactPort,
    ForumTopicReadPostingFactPort, SharedForumAudienceFactsPort,
    SharedForumPostingPolicyOwnerFactPort,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::models::users::{Column as UsersColumn, Entity as UsersEntity};

const INVALID_REQUEST_CODE: &str = "forum.account_age_facts.invalid_request";
const TENANT_MISMATCH_CODE: &str = "forum.account_age_facts.tenant_mismatch";
const ACTOR_MISMATCH_CODE: &str = "forum.account_age_facts.actor_mismatch";
const USER_NOT_FOUND_CODE: &str = "forum.account_age_facts.user_not_found";
const STORAGE_UNAVAILABLE_CODE: &str = "forum.account_age_facts.storage_unavailable";
const STORAGE_INVARIANT_CODE: &str = "forum.account_age_facts.storage_invariant";

pub(crate) type SharedForumPostingPolicyFactsComposer = Arc<ForumPostingPolicyFactsComposer>;

type AccountAgeClock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

/// Host-owned exact-user adapter over the authoritative `users.created_at`
/// timestamp. Forum receives only the derived bounded age in seconds and never
/// imports the server user entity or reads the users table directly.
#[derive(Clone)]
pub(crate) struct ServerForumAccountAgeFactPort {
    db: DatabaseConnection,
    now: AccountAgeClock,
}

impl ServerForumAccountAgeFactPort {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            now: Arc::new(Utc::now),
        }
    }

    #[allow(dead_code)]
    fn with_clock(db: DatabaseConnection, now: AccountAgeClock) -> Self {
        Self { db, now }
    }

    pub(crate) fn shared(db: DatabaseConnection) -> SharedForumPostingPolicyOwnerFactPort {
        Arc::new(Self::new(db))
    }
}

#[async_trait]
impl ForumPostingPolicyOwnerFactPort for ServerForumAccountAgeFactPort {
    fn fact_kind(&self) -> ForumPostingPolicyFactKind {
        ForumPostingPolicyFactKind::AccountAgeSeconds
    }

    async fn resolve_forum_posting_policy_fact(
        &self,
        context: PortContext,
        request: ForumPostingPolicyOwnerFactRequest,
    ) -> Result<ForumPostingPolicyOwnerFactResponse, PortError> {
        let request = request.normalize().map_err(|_| {
            PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum account-age fact request is invalid",
            )
        })?;
        validate_context(&context, request.tenant_id, request.user_id)?;
        if request.fact != ForumPostingPolicyFactKind::AccountAgeSeconds {
            return Err(PortError::validation(
                INVALID_REQUEST_CODE,
                "Forum account-age adapter accepts only account-age facts",
            ));
        }

        let user = UsersEntity::find()
            .filter(UsersColumn::Id.eq(request.user_id))
            .filter(UsersColumn::TenantId.eq(request.tenant_id))
            .one(&self.db)
            .await
            .map_err(|_| {
                PortError::unavailable(
                    STORAGE_UNAVAILABLE_CODE,
                    "Forum account-age owner storage is unavailable",
                )
            })?
            .ok_or_else(|| {
                PortError::not_found(
                    USER_NOT_FOUND_CODE,
                    "Forum account-age owner user was not found",
                )
            })?;

        if user.id != request.user_id || user.tenant_id != request.tenant_id {
            return Err(PortError::invariant_violation(
                STORAGE_INVARIANT_CODE,
                "Forum account-age owner returned a different tenant or user",
            ));
        }

        let observed_at = (self.now)();
        let created_at = user.created_at.with_timezone(&Utc);
        if created_at > observed_at {
            return Err(PortError::invariant_violation(
                STORAGE_INVARIANT_CODE,
                "Forum account-age owner timestamp is later than the observation time",
            ));
        }
        let age_seconds = observed_at.signed_duration_since(created_at).num_seconds();
        let age_seconds = u64::try_from(age_seconds).map_err(|_| {
            PortError::invariant_violation(
                STORAGE_INVARIANT_CODE,
                "Forum account-age owner timestamp could not be represented safely",
            )
        })?;

        Ok(ForumPostingPolicyOwnerFactResponse {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            action: request.action,
            fact: request.fact,
            value: ForumPostingPolicyOwnerFactValue::AccountAgeSeconds(age_seconds),
        })
    }
}

/// Stable host facade that publishes authoritative Forum trust, server-owned
/// account age, Forum-owned topic reading, approved posts and exact topic/reply
/// create-window activity. Other fact kinds remain explicitly unavailable until
/// their owners are added.
pub(crate) struct ServerForumPostingPolicyFactsComposer;

impl ServerForumPostingPolicyFactsComposer {
    pub(crate) fn shared(
        db: DatabaseConnection,
        audience_facts: SharedForumAudienceFactsPort,
    ) -> Result<SharedForumPostingPolicyFactsComposer, PortError> {
        let composer = ForumPostingPolicyFactsComposer::new(vec![
            ForumPostingTrustFactPort::shared(audience_facts),
            ServerForumAccountAgeFactPort::shared(db.clone()),
            ForumApprovedPostsFactPort::shared(db.clone()),
            ForumTopicCreatesWindowFactPort::shared(db.clone()),
            ForumReplyCreatesWindowFactPort::shared(db.clone()),
            ForumTopicReadPostingFactPort::shared(db),
        ])?;
        Ok(Arc::new(composer))
    }
}

fn validate_context(
    context: &PortContext,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())?;
    if context.tenant_id != tenant_id.to_string() {
        return Err(PortError::validation(
            TENANT_MISMATCH_CODE,
            "Forum account-age fact tenant does not match the caller context",
        ));
    }
    if context.actor.kind != PortActorKind::User
        || Uuid::parse_str(&context.actor.id).ok() != Some(user_id)
    {
        return Err(PortError::forbidden(
            ACTOR_MISMATCH_CODE,
            "Forum account-age facts require the exact requested user actor",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_trait::async_trait;
    use chrono::{Duration as ChronoDuration, TimeZone};
    use rustok_api::{PortActor, PortErrorKind};
    use rustok_forum::{
        ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest, ForumPostingAction,
        ForumPostingCandidateMetrics, ForumPostingPolicyCompositionRequest,
        ForumPostingPolicyFactsComposer, ForumPostingPolicyRules, ForumPostingTrustFactPort,
        SharedForumAudienceFactsPort,
    };
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, Database, Set};

    use crate::models::{tenants, users};

    use super::*;

    fn context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::user(user_id.to_string()),
            "en",
            "forum-account-age-facts-test",
        )
        .with_deadline(Duration::from_secs(5))
    }

    fn owner_request(
        tenant_id: Uuid,
        user_id: Uuid,
        action: ForumPostingAction,
    ) -> ForumPostingPolicyOwnerFactRequest {
        ForumPostingPolicyOwnerFactRequest {
            tenant_id,
            user_id,
            action,
            fact: ForumPostingPolicyFactKind::AccountAgeSeconds,
            window_seconds: None,
        }
    }

    fn fixed_clock(now: DateTime<Utc>) -> AccountAgeClock {
        Arc::new(move || now.clone())
    }

    async fn insert_user(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        user_id: Uuid,
        created_at: DateTime<Utc>,
    ) {
        let mut tenant = tenants::ActiveModel::new(
            "Forum account-age test tenant",
            &format!("forum-account-age-{tenant_id}"),
        );
        tenant.id = Set(tenant_id);
        tenant
            .insert(db)
            .await
            .expect("insert account-age test tenant");

        let mut user = users::ActiveModel::new(
            tenant_id,
            &format!("{user_id}@example.com"),
            "test-password-hash",
        );
        user.id = Set(user_id);
        user.created_at = Set(created_at.into());
        user.updated_at = Set(created_at.into());
        user.insert(db).await.expect("insert account-age test user");
    }

    #[derive(Clone)]
    struct FixedTrustFactsPort {
        level: u8,
    }

    #[async_trait]
    impl ForumAudienceFactsPort for FixedTrustFactsPort {
        async fn resolve_forum_audience_facts(
            &self,
            _context: PortContext,
            request: ForumAudienceFactsRequest,
        ) -> Result<ForumAudienceFacts, PortError> {
            Ok(ForumAudienceFacts {
                tenant_id: request.tenant_id,
                user_id: request.user_id,
                trust_level: Some(self.level),
                channel_memberships: Vec::new(),
                group_memberships: Vec::new(),
            })
        }
    }

    #[tokio::test]
    async fn exact_user_created_at_resolves_exact_account_age_seconds() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let observed_at = Utc
            .with_ymd_and_hms(2026, 7, 28, 12, 0, 0)
            .single()
            .expect("fixed observation time");
        let created_at = observed_at - ChronoDuration::days(3) - ChronoDuration::seconds(17);
        insert_user(&db, tenant_id, user_id, created_at).await;
        let provider = ServerForumAccountAgeFactPort::with_clock(db, fixed_clock(observed_at));

        let response = provider
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                owner_request(tenant_id, user_id, ForumPostingAction::CreateReply),
            )
            .await
            .expect("authoritative account age should resolve");

        assert_eq!(
            response.value,
            ForumPostingPolicyOwnerFactValue::AccountAgeSeconds(259_217)
        );
    }

    #[tokio::test]
    async fn trust_and_account_age_compose_without_synthetic_facts() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let observed_at = Utc
            .with_ymd_and_hms(2026, 7, 28, 12, 0, 0)
            .single()
            .expect("fixed observation time");
        insert_user(
            &db,
            tenant_id,
            user_id,
            observed_at - ChronoDuration::days(10),
        )
        .await;
        let trust: SharedForumAudienceFactsPort = Arc::new(FixedTrustFactsPort { level: 25 });
        let account_age: SharedForumPostingPolicyOwnerFactPort = Arc::new(
            ServerForumAccountAgeFactPort::with_clock(db, fixed_clock(observed_at)),
        );
        let composer = ForumPostingPolicyFactsComposer::new(vec![
            ForumPostingTrustFactPort::shared(trust),
            account_age,
        ])
        .expect("unique authoritative providers should compose");
        let rules = ForumPostingPolicyRules {
            minimum_trust_level: Some(20),
            minimum_account_age_seconds: Some(86_400),
            ..ForumPostingPolicyRules::default()
        };

        let input = composer
            .compose(
                context(tenant_id, user_id),
                &rules,
                ForumPostingPolicyCompositionRequest {
                    tenant_id,
                    user_id,
                    action: ForumPostingAction::CreateTopic,
                    candidate: ForumPostingCandidateMetrics {
                        body_bytes: 128,
                        link_count: 0,
                        mention_count: 0,
                        attachment_count: 0,
                    },
                },
            )
            .await
            .expect("trust and account age should compose");

        assert_eq!(input.facts.trust_level, Some(25));
        assert_eq!(input.facts.account_age_seconds, Some(864_000));
        assert!(input.facts.unavailable_facts.is_empty());
    }

    #[tokio::test]
    async fn missing_exact_user_is_non_retryable_not_found() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let provider = ServerForumAccountAgeFactPort::new(db);

        let error = provider
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                owner_request(tenant_id, user_id, ForumPostingAction::CreateReply),
            )
            .await
            .expect_err("missing user must remain explicit");

        assert_eq!(error.kind, PortErrorKind::NotFound);
        assert!(!error.retryable);
        assert_eq!(error.code, USER_NOT_FOUND_CODE);
    }

    #[tokio::test]
    async fn future_created_at_is_an_invariant_violation() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let observed_at = Utc
            .with_ymd_and_hms(2026, 7, 28, 12, 0, 0)
            .single()
            .expect("fixed observation time");
        insert_user(
            &db,
            tenant_id,
            user_id,
            observed_at + ChronoDuration::milliseconds(1),
        )
        .await;
        let provider = ServerForumAccountAgeFactPort::with_clock(db, fixed_clock(observed_at));

        let error = provider
            .resolve_forum_posting_policy_fact(
                context(tenant_id, user_id),
                owner_request(tenant_id, user_id, ForumPostingAction::CreateReply),
            )
            .await
            .expect_err("future account creation must fail closed");

        assert_eq!(error.kind, PortErrorKind::InvariantViolation);
        assert_eq!(error.code, STORAGE_INVARIANT_CODE);
    }

    #[tokio::test]
    async fn foreign_actor_is_rejected_before_storage_access() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let provider = ServerForumAccountAgeFactPort::new(db);

        let error = provider
            .resolve_forum_posting_policy_fact(
                context(tenant_id, Uuid::new_v4()),
                owner_request(tenant_id, user_id, ForumPostingAction::CreateReply),
            )
            .await
            .expect_err("foreign actor must fail before the missing table is read");

        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert_eq!(error.code, ACTOR_MISMATCH_CODE);
    }
}
