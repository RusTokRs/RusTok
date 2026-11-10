# Groups membership enforcement command contract

Status: **source-ready / maintainer execution pending**

## Scope

`GroupMembershipEnforcementCommandPort` is the first Groups-owned write boundary for bounded membership enforcement. It exposes two single-membership direct actions:

- `suspend_membership`;
- `revoke_membership_suspension`.

Both commands are user-actor operations. The neutral Moderation adapter does **not** impersonate this port. It performs trusted producer receipt/subject/scope validation and reuses the same crate-private Groups owner mutation functions.

## Lock and replay order

The direct command uses one owner transaction and preserves the GROUPS-07 lock protocol:

1. reserve tenant/key command admission before any owner reads;
2. replay an identical `group_command_receipts` result, or reject a changed request/actor/group using the same idempotency key;
3. locate and lock/reserve the group row;
4. lock actor/target memberships in deterministic UUID order when local authority is required;
5. lock their enforcement rows in deterministic membership-ID order;
6. evaluate effective actor authority with the Groups owner clock;
7. compare the exact expected target membership revision;
8. mutate the Groups enforcement projection;
9. bump group version, append audit + semantic event, store the command receipt, and commit together.

PostgreSQL command admission uses a transaction advisory lock; SQLite reserves its
writer through the shared empty receipt UPDATE before opening an owner snapshot.
The subsequent group reservation and membership/enforcement locks retain canonical
aggregate order. Command admission supports PostgreSQL and SQLite; historical
MySQL branches in row-lock helpers do not imply MySQL command support.

## Hierarchy and owner protection

A direct command requires a real user actor. Platform `groups:moderate`/`groups:manage` authority may operate without local membership; otherwise the actor must be an effective active local member.

Local hierarchy is fixed:

- owner -> admin/moderator/member;
- admin -> moderator/member;
- moderator -> member;
- member -> none.

The group owner cannot be suspended or revoked through this command, even through the platform-moderate bypass. Ownership must be transferred first. A user cannot directly target their own membership.

Legacy `status=banned` remains fail-closed and is not rewritten as suspension.

## Revision and idempotency

Every request carries `expected_membership_revision >= 1`. The exact locked `group_memberships.revision` must match before mutation; stale requests return `groups.membership_enforcement_revision_conflict`.

Direct command receipts bind tenant, group, actor, command type, request hash and idempotency key. Exact replay returns the stored result with `replayed=true` before current effective authorization. Reusing a key for another actor, group, command or request conflicts.

The enforcement migration trigger remains the canonical revision bridge: every material enforcement insert/update increments the membership subject revision. The command reads the post-mutation owner state and returns that revision.

## Suspension state

A suspension preserves the membership's stored lifecycle status and writes/refreshes the bounded current `group_membership_enforcements` row with:

- `state=suspended`;
- canonical bounded reason code;
- `source_kind=direct_local` for this port;
- owner-clock `effective_from` and optional future `effective_until`;
- the stored lifecycle status as `restore_status`;
- direct actor provenance;
- no Moderation decision identity.

A currently effective suspension is not silently replaced by a fresh direct command; receipt replay is the idempotent path. An expired or revoked row may be reused by a later suspension and its revisions advance normally.

Direct revoke may only revoke an effective `direct_local` suspension. It cannot remove an active `moderation_decision` enforcement row. Revoke sets `revoked_at` while preserving the original suspension source/actor provenance; the revoking actor is retained in immutable audit/event facts.

## Member-count semantics

`groups.member_count` is explicitly a **stored lifecycle active count**, not an owner-clock effective-enforcement count.

Temporary suspension therefore does not decrement `member_count`, and revoke does not increment it. This avoids a time-driven counter split: an expiring suspension becomes ineffective immediately at `effective_until` without requiring cleanup, so a counter that had been decremented at suspension time could not be restored atomically at expiry.

Every actual enforcement mutation still increments `groups.version`. Consumers that need current authority must use the effective membership resolver; `member_count` is not an authorization signal.

Leave/join continue to own lifecycle-count changes because they mutate stored membership lifecycle.

## Atomic audit and semantic events

Successful suspend/revoke commits the following in one Groups transaction:

- enforcement row mutation;
- trigger-owned membership revision advance;
- group-version advance with unchanged lifecycle member count;
- `group_audit_entries` fact;
- append-only `group_domain_events` membership event;
- direct command receipt.

Migration `m20260808_000009_extend_group_domain_events_for_membership_enforcement` expands the existing event ledger from targeted-invitation-only to the exact valid pairs:

- `invitation / groups.invitation.targeted_created`;
- `membership / groups.membership.suspended`;
- `membership / groups.membership.suspension_revoked`.

The SQLite migration rebuild preserves existing sequence/event IDs and reinstalls invitation plus immutability triggers. Downgrade fails closed while append-only membership events exist instead of deleting history to make an older CHECK constraint fit.

## Moderation adapter seam

The owner mutations retain bounded provenance fields used by the neutral adapter:

- `source_kind=moderation_decision`;
- immutable Moderation decision UUID/hash;
- trusted service/system actor identity;
- no copied Moderation report/case/queue/policy data.

The source-complete adapter consumes neutral contracts exported by
`rustok-moderation` and shared `rustok-outbox` producer receipts. Runtime/replay/race
evidence remains pending; the adapter does not access Moderation owner persistence.
See [the canonical implementation plan](implementation-plan.md) for current evidence.

## Verification

Rust compilation and behavioral tests remain deferred to the maintainer. The Node
source guard has been run; source presence is not runtime evidence. Scoped commands:

```bash
cargo check -p rustok-groups
cargo test -p rustok-groups
node scripts/verify/verify-groups-membership-enforcement-command.mjs
```

No Cargo command, unit/integration test, database migration, Node verifier, formatter, workflow or CI job was executed. PostgreSQL/SQLite runtime, contention, replay, security and transport evidence remains open.
