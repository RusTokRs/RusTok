#!/usr/bin/env node

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";

const scriptPath = path.resolve(
  "scripts/verify/verify-forum-mention-notification-integration.mjs",
);

const paths = {
  contract: "crates/rustok-forum/contracts/forum-mention-notification-integration.json",
  forumSource: "crates/rustok-forum/src/notification_source.rs",
  candidateService: "crates/rustok-notifications/src/candidate.rs",
  candidateContract: "crates/rustok-notifications/contracts/notifications-candidate-policy.json",
  recipientPolicy: "apps/server/src/services/notification_recipient_policy.rs",
  socialGraphContract: "crates/rustok-social-graph/contracts/social-graph-notification-policy.json",
};

function writeFixture(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-mention-notification-"));
  const deferred = [
    "successful PostgreSQL mention/quote runtime proof",
    "bounded moderator audience expansion for forum.mention.audience_added",
    "candidate worker default enablement after health and lag evidence",
    "recipient privacy and source authorization recheck on inbox open",
    "recipient privacy and source authorization recheck before delayed channel delivery",
    "retention purge and reconciliation evidence",
  ];
  if (options.removeModeratorDeferred) {
    deferred.splice(
      deferred.indexOf("bounded moderator audience expansion for forum.mention.audience_added"),
      1,
    );
  }

  writeFixture(
    root,
    paths.contract,
    JSON.stringify(
      {
        schema_version: 1,
        slice: "FORUM-12E/NOTIFY-03B-03H/NOTIFY-07C",
        canonical_plan: "crates/rustok-forum/docs/implementation-plan.md",
        owner: "rustok-forum",
        forum_source_provider: paths.forumSource,
        notifications_candidate_service: paths.candidateService,
        notifications_candidate_contract: paths.candidateContract,
        recipient_policy_runtime: paths.recipientPolicy,
        social_graph_policy_contract: paths.socialGraphContract,
        execution_status: options.claimRuntimeExecution
          ? "maintainer_runtime_verified"
          : "source_locked_pending_maintainer_execution",
        deferred,
        verification: {
          source_verifier:
            "scripts/verify/verify-forum-mention-notification-integration.mjs",
          execution_status: "not_run_by_implementation_agent",
        },
      },
      null,
      2,
    ),
  );

  writeFixture(
    root,
    paths.forumSource,
    `
const USER_MENTION_ADDED_TYPE: &str = "forum.mention.user_added";
fn user_mention_relation_exists() {}
fn load_public_target() { ForumTargetAvailability::Deferred; }
fn resolve() {
  if event.actor_id == Some(payload.mentioned_user_id) {}
  NotificationAudiencePage::try_new(
    vec![NotificationAudienceCandidate { recipient_id: payload.mentioned_user_id }],
    None,
  );
}
fn authorize_target_open() {
  "/modules/forum?category={}&topic={}&reply={}";
  "/modules/forum?category={}&topic={}";
}
${options.enableModeratorAudience ? 'const AUDIENCE: &str = "forum.mention.audience_added";' : ""}
`,
  );

  const candidateGates = options.swapCandidatePrivacyAndAuthorization
    ? `
provider.authorize_target_open(AuthorizeNotificationTargetRequest {
  recipient_id: item.recipient_id,
});
self.policy.evaluate(policy_request).await;
`
    : `
self.policy.evaluate(policy_request).await;
provider.authorize_target_open(AuthorizeNotificationTargetRequest {
  recipient_id: item.recipient_id,
});
`;
  writeFixture(
    root,
    paths.candidateService,
    `
pub trait NotificationRecipientPolicy {}
fn process() {
  NotificationRecipientPolicyDecision::Suppress;
  preference_allows_in_app(&self.db);
  let provider = match self.registry.get(&source) { Some(provider) => provider, None => return };
  ${candidateGates}
  self.persist_final_notification(
  );
}
fn persist_final_notification() {
  let txn = self.db.begin().await?;
  preference_allows_in_app(&txn);
  OnConflict::columns([
    notification::Column::TenantId,
    notification::Column::RecipientId,
    notification::Column::SourceSlug,
    notification::Column::SourceEventId,
    notification::Column::NotificationType,
  ]);
  notification::Entity::insert(active);
  ensure_notification_identity();
  status: Set(FanoutItemStatus::Processed);
  txn.commit().await?;
}
`,
  );

  writeFixture(
    root,
    paths.candidateContract,
    JSON.stringify(
      {
        recipient_privacy_policy: {
          allow_all_default_forbidden: !options.allowAllPrivacy,
          production_runtime_composition_delivered: true,
        },
        source_authorization: {
          authorize_target_open_before_creation: true,
        },
        final_notification: {
          semantic_replay_equality_required: true,
          candidate_completion_same_transaction: true,
        },
        upstream_runtime: {
          default_enabled: false,
        },
      },
      null,
      2,
    ),
  );

  const relationshipChecks = options.swapBlockAndMute
    ? `
.mutes_notification(
NotificationRecipientSuppression::Muted;
.blocks_notification(
NotificationRecipientSuppression::Blocked;
`
    : `
.blocks_notification(
NotificationRecipientSuppression::Blocked;
.mutes_notification(
NotificationRecipientSuppression::Muted;
`;
  writeFixture(
    root,
    paths.recipientPolicy,
    `
fn evaluate() {
  .evaluate_profile_privacy(
  ProfilePrivacyDecision::RecipientUnavailable;
  ProfilePrivacyDecision::Restricted;
  NotificationRecipientSuppression::ProfileRestricted;
  blocks_between;
  source_mutes_target;
  ${relationshipChecks}
  NotificationRecipientPolicyError::retryable;
  with_candidate_worker_enabled(candidate_worker_enabled_from_environment());
  Ok(NotificationRecipientPolicyDecision::Allow)
}
`,
  );

  writeFixture(
    root,
    paths.socialGraphContract,
    JSON.stringify(
      {
        privacy_semantics: {
          block_either_direction_suppresses: true,
          mute_source_to_target_only: true,
        },
        server_composition: {
          profile_then_block_then_mute: true,
          candidate_worker_default_enabled: false,
        },
      },
      null,
      2,
    ),
  );

  return root;
}

function run(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function withFixture(options, assertion) {
  const root = fixture(options);
  try {
    assertion(run(root));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("mention notification verifier accepts the canonical source lock", () => {
  withFixture({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  });
});

test("mention notification verifier rejects candidate gate reorder", () => {
  withFixture({ swapCandidatePrivacyAndAuthorization: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /candidate gates must remain ordered/);
  });
});

test("mention notification verifier rejects recipient privacy reorder", () => {
  withFixture({ swapBlockAndMute: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /profile, block and mute order/);
  });
});

test("mention notification verifier rejects premature moderator audience delivery", () => {
  withFixture({ enableModeratorAudience: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /moderator audience delivery must remain deferred/);
  });
});

test("mention notification verifier requires the moderator deferred boundary", () => {
  withFixture({ removeModeratorDeferred: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing deferred boundary/);
  });
});

test("mention notification verifier rejects unexecuted runtime claims", () => {
  withFixture({ claimRuntimeExecution: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not claim maintainer execution/);
  });
});

test("mention notification verifier rejects allow-all privacy", () => {
  withFixture({ allowAllPrivacy: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /allow-all privacy default must remain forbidden/);
  });
});
