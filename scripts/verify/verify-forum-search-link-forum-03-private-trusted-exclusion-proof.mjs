#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = process.cwd();
const contractPath =
  "crates/rustok-forum/contracts/forum-search-link-forum-03-private-trusted-exclusion-proof.json";
const testPath =
  "apps/server/tests/forum_versioned_invalidation_private_trusted_exclusion.rs";
const docPath =
  "crates/rustok-forum/docs/forum-23b2g2b3d15-private-trusted-exclusion-proof.md";
const categoryOwnerPath =
  "crates/rustok-forum/src/services/category_audience_owner.rs";
const topicOwnerPath =
  "crates/rustok-forum/src/services/topic_audience_owner.rs";
const audienceVisibilityPath =
  "crates/rustok-forum/src/services/topic_audience_visibility.rs";
const routeVisibilityPath =
  "crates/rustok-forum/src/services/topic_visibility.rs";
const publicDiscoveryPath =
  "crates/rustok-forum/src/services/public_discovery.rs";
const channelAuthorityPath =
  "crates/rustok-search/src/storefront_channel_authority.rs";

function read(path) {
  return readFileSync(resolve(root, path), "utf8");
}

function requireIncludes(source, fragments, label) {
  for (const fragment of fragments) {
    assert.ok(
      source.includes(fragment),
      `${label} is missing required source marker: ${fragment}`,
    );
  }
}

function requireArrayIncludes(actual, expected, label) {
  assert.ok(Array.isArray(actual), `${label} must be an array`);
  for (const value of expected) {
    assert.ok(actual.includes(value), `${label} is missing: ${value}`);
  }
}

const contract = JSON.parse(read(contractPath));
const test = read(testPath);
const doc = read(docPath);
const categoryOwner = read(categoryOwnerPath);
const topicOwner = read(topicOwnerPath);
const audienceVisibility = read(audienceVisibilityPath);
const routeVisibility = read(routeVisibilityPath);
const publicDiscovery = read(publicDiscoveryPath);
const channelAuthority = read(channelAuthorityPath);

assert.equal(
  contract.contract,
  "forum_search_link_forum_03_private_trusted_exclusion_proof_v1",
);
assert.equal(contract.task, "FORUM-23B2G2B3D15");
assert.equal(contract.target_link, "LINK-FORUM-03");
assert.equal(contract.coverage, "private_and_trusted_channel_exclusion_only");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.test, testPath);
assert.equal(
  contract.verifier,
  "scripts/verify/verify-forum-search-link-forum-03-private-trusted-exclusion-proof.mjs",
);
assert.equal(
  contract.evidence_artifact,
  "target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json",
);
assert.deepEqual(contract.required_runtime, {
  database: "postgresql",
  broker_used: false,
  host_package: "rustok-server",
  search_inbox: "search_projection_inbox",
  projector: "ForumProjectionReconciler",
  storefront_execution: "execute_forum_storefront_search",
});

requireArrayIncludes(
  contract.required_owner_trace,
  [
    "revisions 1 and 2 are the two real category-create forum-scope invalidations",
    "revisions 3 through 5 are the unrestricted private and trusted topic-create category invalidations",
    "revision 6 is the private topic explicit-user audience invalidation",
    "revision 7 is the trusted category minimum-trust audience invalidation",
    "revision 8 is the trusted topic channel-membership audience invalidation",
  ],
  "required_owner_trace",
);
requireArrayIncludes(
  contract.fail_closed_requirements,
  [
    "all category topic and audience mutations use exported Forum owner services",
    "the eight owner revisions are contiguous and have exact target types and target identifiers",
    "the exact typed envelopes enter Search through ForumSearchContractIngress rather than manual inbox insertion",
    "the explicit-user private topic and the trusted-channel topic are absent from the legitimate public Search projection",
    "stale Search rows are injected only after projection to exercise current owner reauthorization rather than simulate Forum writes",
    "the trusted-channel decision is a conjunction of exact route channel inherited trust floor and topic channel membership",
    "trusted storefront route contexts carry both a non-nil channel identifier and the matching slug",
    "owner-facts requests are bounded to the exact requested trust or channel subset and carry deadline semantics",
    "the evidence artifact is written only after every assertion succeeds and the isolated schema is removed",
  ],
  "fail_closed_requirements",
);
requireArrayIncludes(
  contract.non_claims,
  [
    "this source-ready slice does not claim that the verifier PostgreSQL test or storefront execution ran",
    "this proof uses complete internally consistent trusted RequestContext values but does not independently authenticate host Channel resolution",
    "this proof does not execute or simulate topic move because the owner command remains planned under FORUM-21",
    "this proof does not use external Iggy or replace the reviewed D0 D12 D13 or D14 artifacts",
    "this proof is not sufficient to mark LINK-FORUM-03 done or promote FORUM-23",
  ],
  "non_claims",
);
assert.deepEqual(contract.maintainer_commands, [
  "node scripts/verify/verify-forum-search-link-forum-03-private-trusted-exclusion-proof.mjs",
  'RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" cargo test -p rustok-server --test forum_versioned_invalidation_private_trusted_exclusion -- --nocapture --test-threads=1',
]);

requireIncludes(
  test,
  [
    'const EVIDENCE_CONTRACT: &str =\n    "forum_search_link_forum_03_private_trusted_exclusion_proof_v1";',
    '"target/forum-search-link-forum-03-private-trusted-exclusion-evidence.json"',
    'const PUBLIC_TOPIC_MARKER: &str = "d15publictopicmarker";',
    'const PRIVATE_TOPIC_MARKER: &str = "d15privatetopicmarker";',
    'const TRUSTED_TOPIC_MARKER: &str = "d15trustedtopicmarker";',
    'const TRUSTED_CHANNEL: &str = "trusted";',
    'const MINIMUM_TRUST: u8 = 50;',
    'CREATE SCHEMA "{schema_name}"',
    "for migration in OutboxModule.migrations()",
    "for migration in TaxonomyModule.migrations()",
    "for migration in ForumModule.migrations()",
    "for migration in SearchModule.migrations()",
    "CategoryService::new(db.clone())",
    "TopicService::new(db.clone(), bus)",
    "ForumTopicAudiencePolicyService::new(db.clone())",
    "ForumCategoryAudiencePolicyService::new(db.clone())",
    "allow_user_ids: vec![private_user_id]",
    "minimum_trust_level: Some(MINIMUM_TRUST)",
    "channel_members_any: vec![TRUSTED_CHANNEL.to_string()]",
    "Some(vec![TRUSTED_CHANNEL.to_string()])",
    "ForumSearchContractIngress::new(db.clone())",
    "ForumSearchProjectionSourceFactory.build(db.clone())",
    "ForumProjectionReconciler::new(db.clone(), projection_source)",
    "execute_forum_storefront_search(",
    "ForumSearchResultEligibilityService::new(self.db.clone())",
    "ForumSearchResultEligibilityService::with_audience_facts(",
    ".with_deadline(Duration::from_secs(5))",
    "legitimate_private_topic_documents",
    "legitimate_trusted_topic_documents",
    "insert_stale_topic_documents(db, fixture).await?",
    '"public_private_denied"',
    '"private_explicit_user_allowed"',
    '"private_outsider_denied"',
    '"public_trusted_denied"',
    '"trusted_low_trust_denied"',
    '"trusted_nonmember_denied"',
    '"trusted_wrong_route_denied"',
    '"trusted_exact_member_allowed"',
    "channel_id,",
    "channel_slug: channel_slug.map(str::to_string)",
    "count_inbox_rows(db, revision.event_id).await? != 1",
    "caught_up.claimed_events != 0",
    '"owner_revision_compared_to_ingest_sequence": false',
    '"topic_move_executed": false',
    '"topic_move_blocked_on": "FORUM-21"',
    '.args(["rev-parse", "HEAD"])',
    "let scenario = proof?;\n    cleanup?;\n\n    write_evidence",
  ],
  "D15 test",
);

const revisionMarkers = [
  '(1, "forum", None)',
  '(2, "forum", None)',
  '(3, "forum_category", Some(fixture.public_category_id))',
  '(4, "forum_category", Some(fixture.public_category_id))',
  '(5, "forum_category", Some(fixture.trusted_category_id))',
  '(6, "forum_topic", Some(fixture.private_topic_id))',
  '(7, "forum", None)',
  '(8, "forum_topic", Some(fixture.trusted_topic_id))',
];
requireIncludes(test, revisionMarkers, "D15 exact owner revision sequence");

const legitimateCountPosition = test.indexOf(
  "let legitimate_private_topic_documents =",
);
const staleInjectionPosition = test.indexOf(
  "insert_stale_topic_documents(db, fixture).await?",
);
assert.ok(legitimateCountPosition >= 0 && staleInjectionPosition >= 0);
assert.ok(
  legitimateCountPosition < staleInjectionPosition,
  "legitimate restricted-document counts must be captured before stale injection",
);

const staleFunctionStart = test.indexOf("async fn insert_stale_topic_documents");
const storefrontFunctionStart = test.indexOf("async fn assert_storefront_exact");
assert.ok(staleFunctionStart >= 0 && storefrontFunctionStart > staleFunctionStart);
const staleFunction = test.slice(staleFunctionStart, storefrontFunctionStart);
requireIncludes(
  staleFunction,
  [
    "INSERT INTO search_documents",
    '"owner_state": owner_state',
    '"channel_slugs": channel_slugs.clone()',
    '"channel_slugs": channel_slugs,',
  ],
  "controlled stale Search injection",
);
for (const forbiddenOwnerTable of [
  "forum_categories",
  "forum_topics",
  "forum_topic_translations",
  "forum_topic_channel_access",
  "forum_category_audience_policy",
  "forum_topic_audience_policy",
]) {
  assert.ok(
    !staleFunction.includes(`INSERT INTO ${forbiddenOwnerTable}`) &&
      !staleFunction.includes(`UPDATE ${forbiddenOwnerTable}`) &&
      !staleFunction.includes(`DELETE FROM ${forbiddenOwnerTable}`),
    `controlled stale injection must not mutate Forum owner table ${forbiddenOwnerTable}`,
  );
}

assert.ok(
  !test.includes("RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS") &&
    !test.includes("IggyTransport") &&
    !test.includes("PersistentContractConsumerGroup"),
  "D15 must remain a broker-free PostgreSQL proof",
);
assert.ok(
  !test.includes("UPDATE forum_topics SET category_id") &&
    !test.includes("topic_move_executed\": true"),
  "D15 must not simulate topic move",
);
assert.ok(
  !/owner_revision\s*[<>]=?\s*[^\n]*ingest_sequence|ingest_sequence\s*[<>]=?\s*[^\n]*owner_revision/.test(
    test,
  ),
  "D15 must not compare owner_revision numerically with ingest_sequence",
);

requireIncludes(
  categoryOwner,
  [
    "pub struct ForumCategoryAudiencePolicyOwnerService",
    "publish_forum_projection_scope_direct_in_tx",
    "txn.commit().await?",
  ],
  "category audience owner",
);
requireIncludes(
  topicOwner,
  [
    "pub struct ForumTopicAudiencePolicyOwnerService",
    "publish_forum_topic_projection_direct_in_tx",
    "txn.commit().await?",
  ],
  "topic audience owner",
);
requireIncludes(
  routeVisibility,
  [
    "matching_topic_channel_access_subquery",
    "ForumTopicVisibilityScope::storefront_for_viewer",
    "forum_topic::Column::Status.eq(TopicStatus::Open)",
  ],
  "route visibility owner",
);
requireIncludes(
  audienceVisibility,
  [
    "ForumTopicVisibilityService::new(self.db.clone())",
    "for layer in &policy.inherited_category_layers",
    "if let Some(constraints) = &policy.configured_constraints",
    "resolve_for_constraints",
  ],
  "richer audience visibility owner",
);
requireIncludes(
  publicDiscovery,
  [
    "The owner deliberately exposes only the anonymous public decision",
    "get_public_topic_with_locale_fallback",
    "get_public_reply_with_locale_fallback",
  ],
  "public discovery owner",
);
requireIncludes(
  channelAuthority,
  [
    "(Some(channel_id), Some(channel_slug))",
    "IncompleteTrustedChannelContext",
    "RequestedChannelMismatch",
    "RequestTenantMismatch",
  ],
  "trusted storefront channel authority",
);

requireIncludes(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "Why route channel alone is not privacy",
    "Exact owner revision trace",
    "Legitimate projection result",
    "Stale-candidate reauthorization matrix",
    "Owner facts boundary",
    "Topic move remains blocked on the planned `FORUM-21` owner command",
    "No command above was run by the implementation agent",
  ],
  "D15 handoff",
);

console.log(
  "FORUM-23B2G2B3D15 private/trusted Search exclusion source contract verified",
);
