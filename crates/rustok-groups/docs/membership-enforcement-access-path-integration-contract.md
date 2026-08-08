# Groups membership-enforcement access-path integration contract

Status: **Forum audience provider ACL source delivered / maintainer execution pending**

## Scope

This contract records the first external provider ACL consumer that is already wired to Groups owner-clock effective membership: the Forum audience group-facts adapter.

It does **not** claim that every planned provider ACL profile is complete. Blog, Pages/Wiki, Marketplace, Media, Events, Chat, remote/degraded profiles and future group-space integrations remain separate work. FBA `membership_enforcement_access_path_integration` stays null until runtime execution.

## Owner boundary

`apps/server/src/services/forum_audience_group_facts.rs` is a server composition adapter, not a Groups state owner. For every exact requested group fact it constructs a tenant/user-bound read `PortContext` and calls only:

`GroupMembershipEnforcementReadPort::read_membership_enforcement`

The adapter treats only `GroupMembershipEffectiveStatus::Active` as a positive membership fact. Suspended, inactive, legacy-banned or missing membership is not reported as active. Expiry is therefore inherited from the Groups owner clock and does not require a Forum cleanup job.

The adapter validates returned tenant, group and user identity before trusting the owner response. It contains no direct Groups entity/table access and no local reconstruction of enforcement projection state.

## Host composition

`apps/server/src/services/module_event_dispatcher.rs` publishes `ServerForumAudienceGroupFactsPort` only when both `mod-forum` and `mod-groups` are selected. The provider is passed into `ServerForumAudienceFactsPort`, which is inserted into module runtime extensions before downstream Forum notification/posting-policy consumers materialize.

The integration therefore remains host composition through a neutral Groups read port. Forum does not depend on Groups persistence ownership.

## Owner-backed backend source

`apps/server/src/services/forum_audience_group_facts/owner_backed_tests.rs` retains backend-specific owner-backed evidence using the real Groups enforcement command and read paths.

### SQLite

`forum_group_facts_follow_groups_owner_clock_sqlite` uses a real file-backed SQLite database and production Groups migrations. The fixture establishes an active group member, confirms a positive Forum group fact, suspends the member through `GroupMembershipEnforcementCommandPort`, confirms the Forum fact becomes false, then lets `effective_until` expire and confirms the fact becomes true again without revoke or cleanup.

### PostgreSQL

`forum_group_facts_follow_groups_owner_clock_postgres` mirrors the same contract in a unique PostgreSQL schema and is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured.

Both sources prove that the external consumer follows the Groups owner clock rather than raw stored lifecycle status.

## Degraded and partial-provider semantics

Forum audience composition is intentionally a partial provider. A positive requested group fact may decide the positive-selector union. If requested groups do not decide the result and another required membership dimension is unavailable, the composition returns typed retryable unavailable instead of inventing a false deny.

This preserves fail-closed behavior without making the Groups adapter responsible for Forum trust or profile policy.

## Groups contract accounting

`crates/rustok-groups/contracts/groups-effective-membership-access.json` records `forum_audience_group_facts` as `source_delivered_execution_pending` and keeps `additional_provider_specific_acl_adapters` in remaining work.

The broad Groups FBA field `membership_enforcement.provider_acl_integration` remains open because one delivered Forum consumer does not complete all provider profiles. `evidence.membership_enforcement_access_path_integration` remains null because the owner-backed commands were not executed by this implementation agent.

## Upstream Forum evidence

The authoritative Forum-side composition contract is:

`crates/rustok-forum/contracts/forum-audience-group-facts-host-runtime.json`

Its source verifier remains:

`node scripts/verify/verify-forum-audience-group-facts-host-runtime.mjs`

The Groups-side cross-module verifier is:

`node scripts/verify/verify-groups-membership-enforcement-access-path-integration.mjs`

## Execution status

No Cargo command, test, Node verifier, formatter, migration execution, workflow, browser/schema execution, or CI job was run while adding this Groups-side evidence linkage.
