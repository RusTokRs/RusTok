# Forum owner-write acknowledgements do not re-read hidden content

- Date: 2026-10-28
- Decision status: Accepted
- Implementation status: In progress (source-ready, runtime unverified)
- Owner: `rustok-forum` (vote/solution owner services, REST and GraphQL)
- Supersedes: [topic owner audience read](./2026-10-09-forum-topic-owner-audience-read.md) (solution response only); [vote and subscription write audience](./2026-10-09-forum-vote-subscription-write-audience.md) (vote response only)
- Extends: [anonymous discovery and category counts](./2026-10-15-forum-anonymous-discovery-and-category-counts.md) (blind subscription clear acknowledgements)

## Decision

Vote set/clear for topics and replies, and solution mark/clear are commands,
not selected-object reads. The existing post-commit read can reject the
caller under an audience policy that differs from the authorized write and
produce 404 **after the write has already committed**.

These six commands return only the owning service's successful completion:

- REST: `204 No Content`, no body.
- GraphQL: `Boolean!` with `true` after a successful commit.
- Vote set retains optional locale only to construct the existing canonical
  owner-audience write context; vote clear and solution commands no longer
  accept an unused locale parameter.
- Owner failures and permission denials are still errors. No success is
  synthesized on validation, authorization, database, or outbox failure.
- An exact author may still mark/clear a solution through the existing
  moderation policy even when the audience read would deny them. The
  acknowledgment does not contain the hidden topic.
- Vote clear remains an actor-scoped blind delete: hidden/absent targets
  have identical results. Neither vote clear may lock or load its subject
  row: both delete only the tenant/subject/user tuple. Topic votes can be
  cleared after archival. A no-op clear emits no topic projection invalidation.
  Full object details require a **new** authorized
  read, which can legitimately fail after a successful command.

## Transaction, tenant, channel and consumers

This decision changes only transport result shapes; it does not rewrite
Forum-owned vote/solution state or outbox/projection transactions. All
explicit GraphQL vote tenant inputs are checked against `TenantContext`
before the owner service write (vote set previously omitted the check).
Existing RBAC and write-audience rules remain intact; the separate
vote/subscription TOCTOU and channel exclusions remain open issues.

The shape change is deliberate in this unreleased codebase. No legacy
response DTO or alias is retained. First-party Next.js forum APIs, admin
and Leptos forum transports in this source snapshot do not invoke these
six mutations, but independent external consumers must update query
selection sets. Frontends refreshing after a mutation must use a fresh
viewer-authorized query, not an unscoped post-write projection.

## Verification

- `cargo check --locked -p rustok-forum --lib`; scoped GraphQL
  tenant-scope and vote/solution tests; REST status and body assertions.
- Verify a restricted-topic author receives an acknowledgement after an
  authorized solution write while separate owner read remains denied.
- Verify tenant mismatches are refused before data mutation and a hidden
  or absent vote-clear target is never probed or disclosed.
- Verify all six GraphQL fields return `Boolean!` and 204 REST has no body.
- Race-test vote-set audience policy revocation separately under the
  transactional authorization ADR. No runtime tests were run here.
