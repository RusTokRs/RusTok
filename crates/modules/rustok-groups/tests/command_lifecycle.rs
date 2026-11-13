//! Behavioral regressions through public owner ports and production migrations.
use rustok_api::{PortActor, PortContext};
use rustok_core::MigrationSource;
use rustok_groups::*;
use rustok_outbox::OutboxModule;
use sea_orm::{ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter};
use sea_orm_migration::SchemaManager;
use std::time::Duration;
use uuid::Uuid;

struct Fixture {
    db: DatabaseConnection,
    tenant: Uuid,
    owner: Uuid,
    candidate: Uuid,
    group: Uuid,
}
fn context(tenant: Uuid, user: Uuid, key: &str) -> PortContext {
    PortContext::new(
        tenant.to_string(),
        PortActor::user(user.to_string()),
        "en",
        Uuid::new_v4().to_string(),
    )
    .with_deadline(Duration::from_secs(30))
    .with_idempotency_key(key)
}
fn input(policy: GroupJoinPolicy) -> CreateGroupInput {
    CreateGroupInput {
        handle: "regression-group".into(),
        locale: "en".into(),
        title: "Regression group".into(),
        summary: None,
        body: Some("Private body".into()),
        visibility: GroupVisibility::Public,
        join_policy: policy,
        category_id: None,
        avatar_media_id: None,
        cover_media_id: None,
        metadata: serde_json::json!({}),
    }
}
async fn fixture(policy: GroupJoinPolicy) -> Fixture {
    fixture_on("sqlite::memory:", policy).await
}
async fn fixture_on(url: &str, policy: GroupJoinPolicy) -> Fixture {
    let db = Database::connect(url).await.unwrap();
    let manager = SchemaManager::new(&db);
    for migration in OutboxModule
        .migrations()
        .into_iter()
        .chain(GroupsModule.migrations())
    {
        migration.up(&manager).await.unwrap();
    }
    let tenant = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let candidate = Uuid::new_v4();
    let service = GroupsService::new(db.clone());
    let created = service
        .create_group(context(tenant, owner, "create"), input(policy))
        .await
        .unwrap();
    Fixture {
        db,
        tenant,
        owner,
        candidate,
        group: created.summary.id,
    }
}
impl Fixture {
    fn owner(&self, key: &str) -> PortContext {
        context(self.tenant, self.owner, key)
    }
    fn candidate(&self, key: &str) -> PortContext {
        context(self.tenant, self.candidate, key)
    }
    async fn submit(&self, key: &str) -> SubmitGroupMembershipApplicationResult {
        let service = GroupApplicationService::new(self.db.clone());
        let policy = service
            .read_group_application_policy(
                self.candidate("read"),
                ReadGroupApplicationPolicyRequest {
                    group_id: self.group,
                },
            )
            .await
            .unwrap();
        service
            .submit_group_membership_application_if_current(
                self.candidate(key),
                SubmitGroupMembershipApplicationIfCurrentRequest {
                    expected_policy: GroupApplicationPolicyPrecondition {
                        policy_id: policy.id,
                        revision: policy.revision,
                        locale: policy.locale,
                    },
                    submission: SubmitGroupMembershipApplicationRequest {
                        group_id: self.group,
                        answers: Default::default(),
                        acknowledged_rule_keys: Vec::new(),
                    },
                },
            )
            .await
            .unwrap()
    }
    async fn policy(&self) {
        GroupApplicationService::new(self.db.clone())
            .upsert_group_application_policy_if_current(
                self.owner("policy"),
                UpsertGroupApplicationPolicyIfCurrentRequest {
                    expected_policy: None,
                    policy: UpsertGroupApplicationPolicyRequest {
                        group_id: self.group,
                        locale: "en".into(),
                        enabled: true,
                        questions: Vec::new(),
                        rules: Vec::new(),
                    },
                },
            )
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn create_replays_and_rejects_changed_payload() {
    let f = fixture(GroupJoinPolicy::Open).await;
    let service = GroupsService::new(f.db.clone());
    let replay = service
        .create_group(f.owner("create"), input(GroupJoinPolicy::Open))
        .await
        .unwrap();
    assert_eq!(replay.summary.id, f.group);
    let mut changed = input(GroupJoinPolicy::Open);
    changed.title = "Changed".into();
    assert!(
        service
            .create_group(f.owner("create"), changed)
            .await
            .is_err()
    );
    let other_actor = service
        .create_group(f.candidate("create"), input(GroupJoinPolicy::Open))
        .await;
    assert!(other_actor.is_err());
}

#[tokio::test]
async fn delayed_leave_replay_does_not_remove_rejoined_member() {
    let f = fixture(GroupJoinPolicy::Open).await;
    let service = GroupsService::new(f.db.clone());
    service
        .join_group(f.candidate("join"), JoinGroupRequest { group_id: f.group })
        .await
        .unwrap();
    service
        .leave_group(
            f.candidate("leave"),
            LeaveGroupRequest { group_id: f.group },
        )
        .await
        .unwrap();
    service
        .join_group(
            f.candidate("rejoin"),
            JoinGroupRequest { group_id: f.group },
        )
        .await
        .unwrap();
    let replay = service
        .leave_group(
            f.candidate("leave"),
            LeaveGroupRequest { group_id: f.group },
        )
        .await
        .unwrap();
    assert_eq!(replay.status, GroupMembershipStatus::Left);
    let current = service
        .read_membership(
            f.candidate("read"),
            ReadGroupMembershipRequest {
                group_id: f.group,
                user_id: f.candidate,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.status, GroupMembershipStatus::Active);
}

#[tokio::test]
async fn request_join_does_not_create_orphan_membership() {
    let f = fixture(GroupJoinPolicy::Request).await;
    let service = GroupsService::new(f.db.clone());
    let error = service
        .join_group(f.candidate("join"), JoinGroupRequest { group_id: f.group })
        .await
        .unwrap_err();
    assert_eq!(error.code, "groups.application_required");
    assert!(
        service
            .read_membership(
                f.candidate("read"),
                ReadGroupMembershipRequest {
                    group_id: f.group,
                    user_id: f.candidate
                }
            )
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn approved_candidate_can_apply_again_after_leaving() {
    let f = fixture(GroupJoinPolicy::Request).await;
    f.policy().await;
    let first = f.submit("submit").await;
    GroupApplicationReviewCommandPort::review_group_membership_application(
        &GroupApplicationService::new(f.db.clone()),
        f.owner("approve"),
        ReviewGroupMembershipApplicationRequest {
            application_id: first.application.id,
            decision: GroupApplicationReviewDecision::Approve,
            note: None,
        },
    )
    .await
    .unwrap();
    GroupsService::new(f.db.clone())
        .leave_group(
            f.candidate("leave"),
            LeaveGroupRequest { group_id: f.group },
        )
        .await
        .unwrap();
    let next = f.submit("resubmit").await;
    assert_eq!(next.application.status, GroupApplicationStatus::Pending);
    assert_eq!(next.membership.status, GroupMembershipStatus::Pending);
}

#[tokio::test]
async fn invitation_completes_pending_application_and_leave_cancels_pending_attempt() {
    let f = fixture(GroupJoinPolicy::Request).await;
    f.policy().await;
    f.submit("submit").await;
    let service = GroupInvitationService::new(f.db.clone());
    let invitation = service
        .create_group_invitation(
            f.owner("invite"),
            CreateGroupInvitationRequest {
                group_id: f.group,
                target_user_id: Some(f.candidate),
                expires_in_seconds: 3600,
                max_uses: 1,
            },
        )
        .await
        .unwrap();
    service
        .accept_group_invitation(
            f.candidate("accept"),
            AcceptGroupInvitationRequest {
                token: invitation.token.unwrap(),
            },
        )
        .await
        .unwrap();
    let applications = GroupApplicationService::new(f.db.clone());
    let current = applications
        .read_my_group_membership_application(
            f.candidate("read"),
            ReadMyGroupMembershipApplicationRequest { group_id: f.group },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.status, GroupApplicationStatus::Approved);
    let core = GroupsService::new(f.db.clone());
    core.leave_group(
        f.candidate("leave"),
        LeaveGroupRequest { group_id: f.group },
    )
    .await
    .unwrap();
    f.submit("again").await;
    let before = rustok_groups::entities::group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    core.leave_group(
        f.candidate("cancel-by-leave"),
        LeaveGroupRequest { group_id: f.group },
    )
    .await
    .unwrap();
    let after = rustok_groups::entities::group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.member_count, before.member_count);
    assert_eq!(after.version, before.version + 1);
    let current = applications
        .read_my_group_membership_application(
            f.candidate("read"),
            ReadMyGroupMembershipApplicationRequest { group_id: f.group },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.status, GroupApplicationStatus::Cancelled);
}

#[tokio::test]
async fn roster_hides_candidates_and_exact_read_requires_authority() {
    let f = fixture(GroupJoinPolicy::Request).await;
    f.policy().await;
    f.submit("submit").await;
    let core = GroupsService::new(f.db.clone());
    let outsider = context(f.tenant, Uuid::new_v4(), "read");
    let request = ListGroupMembershipsRequest {
        group_id: f.group,
        page: 1,
        per_page: 50,
    };
    let public = core
        .list_memberships(outsider.clone(), request.clone())
        .await
        .unwrap();
    assert_eq!(public.total, 1);
    assert_eq!(
        core.list_memberships(f.owner("read"), request)
            .await
            .unwrap()
            .total,
        2
    );
    assert!(
        core.read_membership(
            outsider,
            ReadGroupMembershipRequest {
                group_id: f.group,
                user_id: f.candidate
            }
        )
        .await
        .is_err()
    );
    let rows = rustok_groups::entities::membership::Entity::find()
        .filter(rustok_groups::entities::membership::Column::GroupId.eq(f.group))
        .all(&f.db)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn concurrent_invitation_replay_returns_same_committed_result() {
    let path = std::env::temp_dir().join(format!("groups-replay-{}.sqlite", Uuid::new_v4()));
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let f = fixture_on(&url, GroupJoinPolicy::Open).await;
    let invitation_service = GroupInvitationService::new(f.db.clone());
    let invitation = invitation_service
        .create_group_invitation(
            f.owner("invite"),
            CreateGroupInvitationRequest {
                group_id: f.group,
                target_user_id: None,
                expires_in_seconds: 3600,
                max_uses: 1,
            },
        )
        .await
        .unwrap();
    let other_db = Database::connect(&url).await.unwrap();
    let other_service = GroupInvitationService::new(other_db.clone());
    let request = AcceptGroupInvitationRequest {
        token: invitation.token.unwrap(),
    };
    let (first, second) = tokio::join!(
        invitation_service.accept_group_invitation(f.candidate("accept"), request.clone()),
        other_service.accept_group_invitation(f.candidate("accept"), request),
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(first.membership.id, second.membership.id);
    assert_ne!(first.replayed, second.replayed);
    drop(invitation_service);
    drop(other_service);
    other_db.close().await.unwrap();
    f.db.close().await.unwrap();
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn feature_replay_does_not_increment_aggregate_version_twice() {
    let f = fixture(GroupJoinPolicy::Open).await;
    let core = GroupsService::new(f.db.clone());
    let request = SetGroupFeatureRequest {
        group_id: f.group,
        feature_key: "forum.discussions".into(),
        contract_version: "1".into(),
        enabled: true,
        sort_order: 0,
        configuration: serde_json::json!({}),
    };
    let first = core
        .set_group_feature(f.owner("feature"), request.clone())
        .await
        .unwrap();
    let before = rustok_groups::entities::group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    let replay = core
        .set_group_feature(f.owner("feature"), request)
        .await
        .unwrap();
    let after = rustok_groups::entities::group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(after.version, before.version);
}

#[tokio::test]
async fn translation_delete_replays_after_the_row_is_gone() {
    let f = fixture(GroupJoinPolicy::Open).await;
    let localization = GroupLocalizationService::new(f.db.clone());
    localization
        .upsert_group_translation(
            f.owner("translate"),
            UpsertGroupTranslationRequest {
                group_id: f.group,
                locale: "ru".into(),
                title: "Группа".into(),
                summary: None,
                body: None,
            },
        )
        .await
        .unwrap();
    let request = DeleteGroupTranslationRequest {
        group_id: f.group,
        locale: "ru".into(),
    };
    let first = localization
        .delete_group_translation(f.owner("delete-translation"), request.clone())
        .await
        .unwrap();
    let replay = localization
        .delete_group_translation(f.owner("delete-translation"), request)
        .await
        .unwrap();
    assert_eq!(first, replay);
}

#[tokio::test]
async fn secret_application_surfaces_hide_join_policy_and_lifecycle() {
    use rustok_groups::entities::group;
    use sea_orm::{ActiveModelTrait, Set};

    let f = fixture(GroupJoinPolicy::Open).await;
    let service = GroupApplicationService::new(f.db.clone());
    for status in [
        GroupStatus::Active,
        GroupStatus::Archived,
        GroupStatus::Suspended,
    ] {
        for join_policy in [GroupJoinPolicy::InviteOnly, GroupJoinPolicy::Request] {
            let current = group::Entity::find_by_id(f.group)
                .one(&f.db)
                .await
                .unwrap()
                .unwrap();
            let mut active: group::ActiveModel = current.into();
            active.visibility = Set(GroupVisibility::Secret.as_str().into());
            active.join_policy = Set(join_policy.as_str().into());
            active.status = Set(status.as_str().into());
            active.archived_at =
                Set((status == GroupStatus::Archived).then(|| chrono::Utc::now().fixed_offset()));
            active.update(&f.db).await.unwrap();

            let read_error = service
                .read_group_application_policy(
                    f.candidate("read-secret-policy"),
                    ReadGroupApplicationPolicyRequest { group_id: f.group },
                )
                .await
                .unwrap_err();
            assert_eq!(read_error.code, "groups.not_found");
            let current_error = service
                .read_my_group_membership_application(
                    f.candidate("read-secret-application"),
                    ReadMyGroupMembershipApplicationRequest { group_id: f.group },
                )
                .await
                .unwrap_err();
            assert_eq!(current_error.code, "groups.not_found");
            let request = SubmitGroupMembershipApplicationRequest {
                group_id: f.group,
                answers: Default::default(),
                acknowledged_rule_keys: Vec::new(),
            };
            let submit_error = GroupApplicationCommandPort::submit_group_membership_application(
                &service,
                f.candidate("submit-secret"),
                request.clone(),
            )
            .await
            .unwrap_err();
            assert_eq!(submit_error.code, "groups.not_found");
            // Secret visibility must win over both a stale CAS and an unavailable policy.
            let cas_error = service
                .submit_group_membership_application_if_current(
                    f.candidate("submit-secret-cas"),
                    SubmitGroupMembershipApplicationIfCurrentRequest {
                        expected_policy: GroupApplicationPolicyPrecondition {
                            policy_id: Uuid::new_v4(),
                            revision: 1,
                            locale: "en".into(),
                        },
                        submission: request,
                    },
                )
                .await
                .unwrap_err();
            assert_eq!(cas_error.code, "groups.not_found");
        }
    }
}

#[tokio::test]
async fn application_surface_rejects_nil_ids_and_hides_missing_groups() {
    let f = fixture(GroupJoinPolicy::Request).await;
    let service = GroupApplicationService::new(f.db.clone());
    let invalid = service
        .read_group_application_policy(
            f.candidate("read"),
            ReadGroupApplicationPolicyRequest {
                group_id: Uuid::nil(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(invalid.code, "groups.invalid_group_id");
    let missing = service
        .read_my_group_membership_application(
            f.candidate("read"),
            ReadMyGroupMembershipApplicationRequest {
                group_id: Uuid::new_v4(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(missing.code, "groups.not_found");
}

#[tokio::test]
async fn candidate_cancellation_hides_a_group_that_became_secret() {
    use rustok_groups::entities::group;
    use sea_orm::{ActiveModelTrait, Set};

    let f = fixture(GroupJoinPolicy::Request).await;
    f.policy().await;
    let submission = f.submit("submit").await;
    let current = group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    let mut active: group::ActiveModel = current.into();
    active.visibility = Set(GroupVisibility::Secret.as_str().into());
    active.status = Set(GroupStatus::Archived.as_str().into());
    active.archived_at = Set(Some(chrono::Utc::now().fixed_offset()));
    active.update(&f.db).await.unwrap();
    let error = GroupApplicationService::new(f.db.clone())
        .cancel_group_membership_application(
            f.candidate("cancel-secret"),
            CancelGroupMembershipApplicationRequest {
                application_id: submission.application.id,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "groups.not_found");
    let application =
        rustok_groups::application_entities::membership_application::Entity::find_by_id(
            submission.application.id,
        )
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(application.status, GroupApplicationStatus::Pending.as_str());
}

#[tokio::test]
async fn join_aborts_atomically_when_version_or_count_cannot_advance() {
    use rustok_groups::entities::{group, membership};
    use sea_orm::{ActiveModelTrait, Set};

    let f = fixture(GroupJoinPolicy::Open).await;
    let service = GroupsService::new(f.db.clone());
    for (version, member_count) in [(i64::MAX, 1), (1, i64::MAX)] {
        let current = group::Entity::find_by_id(f.group)
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();
        let mut active: group::ActiveModel = current.into();
        active.version = Set(version);
        active.member_count = Set(member_count);
        active.update(&f.db).await.unwrap();
        let error = service
            .join_group(
                f.candidate("overflow-join"),
                JoinGroupRequest { group_id: f.group },
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, "groups.invariant");
        let unchanged = group::Entity::find_by_id(f.group)
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (unchanged.version, unchanged.member_count),
            (version, member_count)
        );
        let candidate = membership::Entity::find()
            .filter(membership::Column::GroupId.eq(f.group))
            .filter(membership::Column::UserId.eq(f.candidate))
            .one(&f.db)
            .await
            .unwrap();
        assert!(candidate.is_none());
    }
    // Failed attempts must not have committed a success receipt under the same key.
    let current = group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    let mut active: group::ActiveModel = current.into();
    active.version = Set(1);
    active.member_count = Set(1);
    active.update(&f.db).await.unwrap();
    service
        .join_group(
            f.candidate("overflow-join"),
            JoinGroupRequest { group_id: f.group },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn direct_join_of_invited_membership_completes_pending_application() {
    use rustok_groups::entities::membership;
    use sea_orm::{ActiveModelTrait, Set};

    let f = fixture(GroupJoinPolicy::Request).await;
    f.policy().await;
    let pending = f.submit("submit").await;
    // Model a stored invitation lifecycle row independently of token redemption.
    let current = membership::Entity::find_by_id(pending.membership.id)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    let mut invited: membership::ActiveModel = current.into();
    invited.status = Set(GroupMembershipStatus::Invited.as_str().into());
    invited.update(&f.db).await.unwrap();
    let core = GroupsService::new(f.db.clone());
    let joined = core
        .join_group(
            f.candidate("join-invited"),
            JoinGroupRequest { group_id: f.group },
        )
        .await
        .unwrap();
    assert_eq!(joined.status, GroupMembershipStatus::Active);
    let application = GroupApplicationService::new(f.db.clone())
        .read_my_group_membership_application(
            f.candidate("read"),
            ReadMyGroupMembershipApplicationRequest { group_id: f.group },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(application.status, GroupApplicationStatus::Approved);
    let group = rustok_groups::entities::group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group.member_count, 2);
    core.join_group(
        f.candidate("join-invited"),
        JoinGroupRequest { group_id: f.group },
    )
    .await
    .unwrap();
    let replayed = rustok_groups::entities::group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (replayed.member_count, replayed.version),
        (group.member_count, group.version)
    );
}

#[tokio::test]
async fn invitation_revocation_replays_without_reading_deleted_invitation() {
    let f = fixture(GroupJoinPolicy::Open).await;
    let service = GroupInvitationService::new(f.db.clone());
    let created = service
        .create_group_invitation(
            f.owner("invite"),
            CreateGroupInvitationRequest {
                group_id: f.group,
                target_user_id: None,
                expires_in_seconds: 3600,
                max_uses: 1,
            },
        )
        .await
        .unwrap();
    let request = RevokeGroupInvitationRequest {
        invitation_id: created.invitation.id,
    };
    let first = service
        .revoke_group_invitation(f.owner("revoke"), request.clone())
        .await
        .unwrap();
    rustok_groups::invitation_entities::invitation::Entity::delete_by_id(created.invitation.id)
        .exec(&f.db)
        .await
        .unwrap();
    let replay = service
        .revoke_group_invitation(f.owner("revoke"), request)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.invitation, first.invitation);
    assert_eq!(replay.group_version, first.group_version);
}

#[tokio::test]
async fn policy_history_does_not_decode_unrelated_application_payloads() {
    use rustok_groups::application_entities::membership_application;
    use sea_orm::{ActiveModelTrait, Set};

    let f = fixture(GroupJoinPolicy::Request).await;
    f.policy().await;
    let submitted = f.submit("submit").await;
    let history = GroupApplicationPolicyHistoryService::new(f.db.clone());
    let request = ListGroupApplicationPolicyRevisionsRequest {
        group_id: f.group,
        page: 1,
        per_page: 20,
    };
    let before = history
        .list_group_application_policy_revisions(f.owner("history"), request.clone())
        .await
        .unwrap();
    assert!(before.total > 0);
    let current = membership_application::Entity::find_by_id(submitted.application.id)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    let mut active: membership_application::ActiveModel = current.into();
    // Storage permits a JSON object, but the numeric answer violates the typed string contract.
    active.answers = Set(serde_json::json!({"invalid": 42}));
    active.update(&f.db).await.unwrap();
    let application_error = GroupApplicationService::new(f.db.clone())
        .list_group_membership_applications(
            f.owner("list"),
            ListGroupMembershipApplicationsRequest {
                group_id: f.group,
                status: None,
                page: 1,
                per_page: 20,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(application_error.code, "groups.invariant");
    let after = history
        .list_group_application_policy_revisions(f.owner("history"), request)
        .await
        .unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn policy_history_keeps_manager_and_tenant_boundaries() {
    let f = fixture(GroupJoinPolicy::Request).await;
    f.policy().await;
    let history = GroupApplicationPolicyHistoryService::new(f.db.clone());
    let request = ListGroupApplicationPolicyRevisionsRequest {
        group_id: f.group,
        page: 1,
        per_page: 20,
    };
    let outsider = history
        .list_group_application_policy_revisions(f.candidate("history"), request.clone())
        .await
        .unwrap_err();
    assert_eq!(outsider.code, "groups.manager_required");
    let other_tenant = history
        .list_group_application_policy_revisions(
            context(Uuid::new_v4(), f.owner, "history"),
            request,
        )
        .await;
    assert!(other_tenant.is_err());
}

#[tokio::test]
async fn application_pages_use_id_to_break_equal_submission_times() {
    use rustok_groups::application_entities::membership_application;
    use sea_orm::{ActiveModelTrait, Set};

    let f = fixture(GroupJoinPolicy::Request).await;
    f.policy().await;
    let timestamp = chrono::Utc::now().fixed_offset();
    let mut expected = Vec::new();
    for index in 0..3 {
        let candidate = Fixture {
            db: f.db.clone(),
            tenant: f.tenant,
            owner: f.owner,
            group: f.group,
            candidate: Uuid::new_v4(),
        };
        let submission = candidate.submit(&format!("submit-{index}")).await;
        let current = membership_application::Entity::find_by_id(submission.application.id)
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();
        let mut active: membership_application::ActiveModel = current.into();
        active.submitted_at = Set(timestamp);
        active.update(&f.db).await.unwrap();
        expected.push(submission.application.id);
    }
    expected.sort_unstable_by(|left, right| right.cmp(left));
    let service = GroupApplicationService::new(f.db.clone());
    for _ in 0..2 {
        let mut actual = Vec::new();
        for page in 1..=4 {
            let result = service
                .list_group_membership_applications(
                    f.owner("read"),
                    ListGroupMembershipApplicationsRequest {
                        group_id: f.group,
                        status: Some(GroupApplicationStatus::Pending),
                        page,
                        per_page: 1,
                    },
                )
                .await
                .unwrap();
            assert_eq!(result.total, 3);
            assert_eq!(result.items.len(), usize::from(page <= 3));
            actual.extend(result.items.into_iter().map(|item| item.id));
        }
        assert_eq!(actual, expected);
    }
}

#[tokio::test]
async fn invitation_pages_use_id_to_break_equal_creation_times() {
    use rustok_groups::invitation_entities::invitation;
    use sea_orm::{ActiveModelTrait, Set};

    let f = fixture(GroupJoinPolicy::Open).await;
    let service = GroupInvitationService::new(f.db.clone());
    let timestamp = chrono::Utc::now().fixed_offset();
    let mut expected = Vec::new();
    for index in 0..3 {
        let created = service
            .create_group_invitation(
                f.owner(&format!("invite-{index}")),
                CreateGroupInvitationRequest {
                    group_id: f.group,
                    target_user_id: None,
                    expires_in_seconds: 3600,
                    max_uses: 1,
                },
            )
            .await
            .unwrap();
        let current = invitation::Entity::find_by_id(created.invitation.id)
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();
        let mut active: invitation::ActiveModel = current.into();
        active.created_at = Set(timestamp);
        active.update(&f.db).await.unwrap();
        expected.push(created.invitation.id);
    }
    expected.sort_unstable_by(|left, right| right.cmp(left));
    for include_inactive in [false, true] {
        let mut actual = Vec::new();
        for page in 1..=4 {
            let result = service
                .list_group_invitations(
                    f.owner("read"),
                    ListGroupInvitationsRequest {
                        group_id: f.group,
                        include_inactive,
                        page,
                        per_page: 1,
                    },
                )
                .await
                .unwrap();
            assert_eq!(result.total, 3);
            assert_eq!(result.items.len(), usize::from(page <= 3));
            actual.extend(result.items.into_iter().map(|item| item.id));
        }
        assert_eq!(actual, expected);
    }
}

#[tokio::test]
async fn manager_commands_authorize_before_disclosing_archived_group_state() {
    use rustok_groups::entities::group;
    use sea_orm::{ActiveModelTrait, Set};

    let f = fixture(GroupJoinPolicy::Request).await;
    f.policy().await;
    let pending = f.submit("submit").await;
    let current = group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    let mut archived: group::ActiveModel = current.into();
    archived.status = Set(GroupStatus::Archived.as_str().into());
    archived.archived_at = Set(Some(chrono::Utc::now().fixed_offset()));
    archived.update(&f.db).await.unwrap();
    let outsider = Uuid::new_v4();
    let applications = GroupApplicationService::new(f.db.clone());
    let policy = UpsertGroupApplicationPolicyRequest {
        group_id: f.group,
        locale: "en".into(),
        enabled: true,
        questions: Vec::new(),
        rules: Vec::new(),
    };
    let mut errors = Vec::new();
    errors.push(
        applications
            .upsert_group_application_policy(context(f.tenant, outsider, "upsert"), policy.clone())
            .await
            .unwrap_err(),
    );
    errors.push(
        applications
            .upsert_group_application_policy_if_current(
                context(f.tenant, outsider, "upsert-cas"),
                UpsertGroupApplicationPolicyIfCurrentRequest {
                    expected_policy: None,
                    policy,
                },
            )
            .await
            .unwrap_err(),
    );
    errors.push(
        GroupApplicationReviewCommandPort::review_group_membership_application(
            &applications,
            context(f.tenant, outsider, "review"),
            ReviewGroupMembershipApplicationRequest {
                application_id: pending.application.id,
                decision: GroupApplicationReviewDecision::Approve,
                note: None,
            },
        )
        .await
        .unwrap_err(),
    );
    errors.push(
        applications
            .reopen_group_membership_application(
                context(f.tenant, outsider, "reopen"),
                ReopenGroupMembershipApplicationRequest {
                    application_id: pending.application.id,
                },
            )
            .await
            .unwrap_err(),
    );
    errors.push(
        GroupInvitationService::new(f.db.clone())
            .create_group_invitation(
                context(f.tenant, outsider, "invite"),
                CreateGroupInvitationRequest {
                    group_id: f.group,
                    target_user_id: None,
                    expires_in_seconds: 3600,
                    max_uses: 1,
                },
            )
            .await
            .unwrap_err(),
    );
    for error in errors {
        assert_eq!(error.code, "groups.manager_required");
    }
    // Authorized managers still receive the real lifecycle conflict.
    let owner_error = GroupApplicationReviewCommandPort::review_group_membership_application(
        &applications,
        f.owner("owner-review"),
        ReviewGroupMembershipApplicationRequest {
            application_id: pending.application.id,
            decision: GroupApplicationReviewDecision::Approve,
            note: None,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(owner_error.code, "groups.conflict");
}

#[tokio::test]
async fn review_requires_pending_membership_as_well_as_pending_application() {
    use rustok_groups::{
        application_entities::membership_application,
        entities::{group, membership},
    };
    use sea_orm::{ActiveModelTrait, Set};

    let f = fixture(GroupJoinPolicy::Request).await;
    f.policy().await;
    let submitted = f.submit("submit").await;
    let service = GroupApplicationService::new(f.db.clone());
    let baseline = group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    // Simulate inconsistent/imported owner state without modifying the application.
    for status in [GroupMembershipStatus::Left, GroupMembershipStatus::Invited] {
        let current = membership::Entity::find_by_id(submitted.membership.id)
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();
        let mut active: membership::ActiveModel = current.into();
        active.status = Set(status.as_str().into());
        active.left_at =
            Set((status == GroupMembershipStatus::Left).then(|| chrono::Utc::now().fixed_offset()));
        active.update(&f.db).await.unwrap();
        for decision in [
            GroupApplicationReviewDecision::Approve,
            GroupApplicationReviewDecision::Reject,
        ] {
            let key = format!("review-{}-{decision:?}", status.as_str());
            let error = GroupApplicationReviewCommandPort::review_group_membership_application(
                &service,
                f.owner(&key),
                ReviewGroupMembershipApplicationRequest {
                    application_id: submitted.application.id,
                    decision,
                    note: None,
                },
            )
            .await
            .unwrap_err();
            assert_eq!(error.code, "groups.conflict");
            assert_eq!(error.message, "membership is no longer pending");
            let unchanged = group::Entity::find_by_id(f.group)
                .one(&f.db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                (unchanged.version, unchanged.member_count),
                (baseline.version, baseline.member_count)
            );
            let application = membership_application::Entity::find_by_id(submitted.application.id)
                .one(&f.db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(application.status, GroupApplicationStatus::Pending.as_str());
            let member = membership::Entity::find_by_id(submitted.membership.id)
                .one(&f.db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(member.status, status.as_str());
        }
    }
    let current = membership::Entity::find_by_id(submitted.membership.id)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    let mut active: membership::ActiveModel = current.into();
    active.status = Set(GroupMembershipStatus::Pending.as_str().into());
    active.left_at = Set(None);
    active.update(&f.db).await.unwrap();
    // A rejected attempt must not leave a success receipt: the same request can be retried.
    let approved = GroupApplicationReviewCommandPort::review_group_membership_application(
        &service,
        f.owner("review-left-Approve"),
        ReviewGroupMembershipApplicationRequest {
            application_id: submitted.application.id,
            decision: GroupApplicationReviewDecision::Approve,
            note: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(approved.membership.status, GroupMembershipStatus::Active);
}

#[tokio::test]
async fn visibility_and_join_policy_admission_matrix() {
    use rustok_groups::entities::{group, membership};
    use sea_orm::{ActiveModelTrait, Set};

    for visibility in [
        GroupVisibility::Public,
        GroupVisibility::Closed,
        GroupVisibility::Secret,
    ] {
        for policy in [
            GroupJoinPolicy::Open,
            GroupJoinPolicy::Request,
            GroupJoinPolicy::InviteOnly,
        ] {
            let f = fixture(policy).await;
            let current = group::Entity::find_by_id(f.group)
                .one(&f.db)
                .await
                .unwrap()
                .unwrap();
            let mut active: group::ActiveModel = current.into();
            active.visibility = Set(visibility.as_str().into());
            active.update(&f.db).await.unwrap();
            let core = GroupsService::new(f.db.clone());
            let details_request = ReadGroupRequest {
                group_id: Some(f.group),
                handle: None,
            };
            if visibility == GroupVisibility::Closed {
                let before = core
                    .read_group(f.candidate("read"), details_request.clone())
                    .await
                    .unwrap();
                assert!(before.body.is_none());
            }
            let result = core
                .join_group(f.candidate("join"), JoinGroupRequest { group_id: f.group })
                .await;
            let direct = visibility != GroupVisibility::Secret && policy == GroupJoinPolicy::Open;
            if direct {
                let joined = result.unwrap();
                assert_eq!(joined.status, GroupMembershipStatus::Active);
                let replay = core
                    .join_group(f.candidate("join"), JoinGroupRequest { group_id: f.group })
                    .await
                    .unwrap();
                assert_eq!(joined, replay);
                let after = core
                    .read_group(f.candidate("read"), details_request)
                    .await
                    .unwrap();
                assert_eq!(after.body.as_deref(), Some("Private body"));
            } else {
                let error = result.unwrap_err();
                let code = if visibility != GroupVisibility::Secret
                    && policy == GroupJoinPolicy::Request
                {
                    "groups.application_required"
                } else {
                    "groups.forbidden"
                };
                assert_eq!(error.code, code);
            }
            let stored = group::Entity::find_by_id(f.group)
                .one(&f.db)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(stored.member_count, if direct { 2 } else { 1 });
            let candidate = membership::Entity::find()
                .filter(membership::Column::GroupId.eq(f.group))
                .filter(membership::Column::UserId.eq(f.candidate))
                .one(&f.db)
                .await
                .unwrap();
            assert_eq!(candidate.is_some(), direct);
        }
    }
}

#[tokio::test]
async fn application_commands_validate_raw_dtos_before_storage_or_normalization() {
    // Deliberately no schema: invalid DTOs must be rejected before any owner/receipt query.
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let service = GroupApplicationService::new(db);
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let ctx = || context(tenant, actor, "invalid-input");
    let policy = UpsertGroupApplicationPolicyRequest {
        group_id: Uuid::nil(),
        locale: "en".into(),
        enabled: true,
        questions: Vec::new(),
        rules: Vec::new(),
    };
    let mut errors = Vec::new();
    errors.push(
        service
            .upsert_group_application_policy(ctx(), policy.clone())
            .await
            .unwrap_err(),
    );
    errors.push(
        service
            .upsert_group_application_policy_if_current(
                ctx(),
                UpsertGroupApplicationPolicyIfCurrentRequest {
                    expected_policy: None,
                    policy,
                },
            )
            .await
            .unwrap_err(),
    );
    let expected_policy = GroupApplicationPolicyPrecondition {
        policy_id: Uuid::new_v4(),
        revision: 1,
        locale: "en".into(),
    };
    for submission in [
        SubmitGroupMembershipApplicationRequest {
            group_id: Uuid::nil(),
            answers: Default::default(),
            acknowledged_rule_keys: Vec::new(),
        },
        SubmitGroupMembershipApplicationRequest {
            group_id: Uuid::new_v4(),
            answers: Default::default(),
            acknowledged_rule_keys: vec!["same-rule".into(); 21],
        },
    ] {
        errors.push(
            service
                .submit_group_membership_application(ctx(), submission.clone())
                .await
                .unwrap_err(),
        );
        errors.push(
            service
                .submit_group_membership_application_if_current(
                    ctx(),
                    SubmitGroupMembershipApplicationIfCurrentRequest {
                        expected_policy: expected_policy.clone(),
                        submission,
                    },
                )
                .await
                .unwrap_err(),
        );
    }
    for request in [
        ReviewGroupMembershipApplicationRequest {
            application_id: Uuid::nil(),
            decision: GroupApplicationReviewDecision::Approve,
            note: None,
        },
        ReviewGroupMembershipApplicationRequest {
            application_id: Uuid::new_v4(),
            decision: GroupApplicationReviewDecision::Reject,
            note: Some(" ".repeat(2001)),
        },
    ] {
        errors.push(
            GroupApplicationReviewCommandPort::review_group_membership_application(
                &service,
                ctx(),
                request.clone(),
            )
            .await
            .unwrap_err(),
        );
        errors.push(
            GroupApplicationCommandPort::review_group_membership_application(
                &service,
                ctx(),
                request,
            )
            .await
            .unwrap_err(),
        );
    }
    for error in errors {
        assert_eq!(
            error.code, "groups.validation",
            "unexpected storage or normalization path: {error:?}"
        );
    }
}

#[tokio::test]
async fn lifecycle_and_invitation_nil_ids_fail_before_storage() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let ctx = || context(tenant, actor, "invalid-id");
    let applications = GroupApplicationService::new(db.clone());
    let errors = [
        applications
            .cancel_group_membership_application(
                ctx(),
                CancelGroupMembershipApplicationRequest {
                    application_id: Uuid::nil(),
                },
            )
            .await
            .unwrap_err(),
        applications
            .reopen_group_membership_application(
                ctx(),
                ReopenGroupMembershipApplicationRequest {
                    application_id: Uuid::nil(),
                },
            )
            .await
            .unwrap_err(),
        GroupInvitationService::new(db.clone())
            .revoke_group_invitation(
                ctx(),
                RevokeGroupInvitationRequest {
                    invitation_id: Uuid::nil(),
                },
            )
            .await
            .unwrap_err(),
        GroupTargetedInvitationService::new(db)
            .accept_targeted_group_invitation(
                ctx(),
                AcceptTargetedGroupInvitationRequest {
                    invitation_id: Uuid::nil(),
                },
            )
            .await
            .unwrap_err(),
    ];
    for error in errors {
        assert_eq!(error.code, "groups.validation");
        assert!(error.message.ends_with("cannot be nil UUID"));
    }
}

#[tokio::test]
async fn summary_read_rejects_corrupt_negative_member_count() {
    use rustok_groups::entities::group;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, DbBackend, Set, Statement};

    let f = fixture(GroupJoinPolicy::Open).await;
    // Bypass production CHECK constraints only to exercise the read projection's corruption boundary.
    f.db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "PRAGMA ignore_check_constraints = ON",
    ))
    .await
    .unwrap();
    let current = group::Entity::find_by_id(f.group)
        .one(&f.db)
        .await
        .unwrap()
        .unwrap();
    let mut active: group::ActiveModel = current.into();
    active.member_count = Set(-1);
    active.update(&f.db).await.unwrap();
    let error = GroupsService::new(f.db.clone())
        .read_group(
            f.owner("read"),
            ReadGroupRequest {
                group_id: Some(f.group),
                handle: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, "groups.invariant");
}

#[tokio::test]
async fn corrupt_invitation_uses_cannot_be_repaired_by_acceptance() {
    use rustok_groups::entities::{group, membership};
    use rustok_groups::invitation_entities::{invitation, redemption};
    use sea_orm::{ActiveModelTrait, ConnectionTrait, DbBackend, Set, Statement};

    for (targeted, max_uses, use_count) in [(true, 1, -1), (false, 1, -1), (false, 101, 0)] {
        let f = fixture(GroupJoinPolicy::InviteOnly).await;
        let service = GroupInvitationService::new(f.db.clone());
        let created = service
            .create_group_invitation(
                f.owner("invite"),
                CreateGroupInvitationRequest {
                    group_id: f.group,
                    target_user_id: targeted.then_some(f.candidate),
                    expires_in_seconds: 3600,
                    max_uses: 1,
                },
            )
            .await
            .unwrap();
        f.db.execute_raw(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA ignore_check_constraints = ON",
        ))
        .await
        .unwrap();
        let original = invitation::Entity::find_by_id(created.invitation.id)
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();
        let mut corrupt: invitation::ActiveModel = original.clone().into();
        corrupt.max_uses = Set(max_uses);
        corrupt.use_count = Set(use_count);
        corrupt.update(&f.db).await.unwrap();
        let before = group::Entity::find_by_id(f.group)
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();

        let accept = || async {
            if targeted {
                GroupTargetedInvitationService::new(f.db.clone())
                    .accept_targeted_group_invitation(
                        f.candidate("accept"),
                        AcceptTargetedGroupInvitationRequest {
                            invitation_id: created.invitation.id,
                        },
                    )
                    .await
            } else {
                service
                    .accept_group_invitation(
                        f.candidate("accept"),
                        AcceptGroupInvitationRequest {
                            token: created.token.clone().unwrap(),
                        },
                    )
                    .await
            }
        };
        assert_eq!(accept().await.unwrap_err().code, "groups.invariant");
        let after = group::Entity::find_by_id(f.group)
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.member_count, before.member_count);
        assert_eq!(after.version, before.version);
        assert!(
            membership::Entity::find()
                .filter(membership::Column::GroupId.eq(f.group))
                .filter(membership::Column::UserId.eq(f.candidate))
                .one(&f.db)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            redemption::Entity::find()
                .filter(redemption::Column::InvitationId.eq(created.invitation.id))
                .one(&f.db)
                .await
                .unwrap()
                .is_none()
        );
        let unchanged = invitation::Entity::find_by_id(created.invitation.id)
            .one(&f.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(unchanged.max_uses, max_uses);
        assert_eq!(unchanged.use_count, use_count);

        // Repair is explicit, not an acceptance side effect. The failed command left no receipt.
        let mut repaired: invitation::ActiveModel = unchanged.into();
        repaired.max_uses = Set(original.max_uses);
        repaired.use_count = Set(original.use_count);
        repaired.update(&f.db).await.unwrap();
        assert!(!accept().await.unwrap().replayed);
        assert!(accept().await.unwrap().replayed);
    }
}
