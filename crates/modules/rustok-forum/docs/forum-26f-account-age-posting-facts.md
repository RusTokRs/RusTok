# FORUM-26F account-age posting facts

Status: source-ready / unvalidated.

## Delivered

- `ServerForumAccountAgeFactPort` implements the public FORUM-26E owner-fact SPI for `AccountAgeSeconds`.
- The host reads the authoritative `users.created_at` timestamp by the exact `(tenant_id, user_id)` pair. The Forum crate does not import the server user entity or query the users table.
- Every direct provider call requires read deadline semantics, an exact tenant string and an exact `PortActorKind::User` UUID before storage access.
- The provider captures one observation timestamp per call. Production uses `Utc::now`; source proof injects a fixed clock so exact second calculations and future-timestamp behavior are deterministic.
- A timestamp later than the observation time is an invariant violation. It is never clamped to zero.
- A missing exact user returns typed non-retryable `NotFound`. The existing FORUM-26E composer converts that capability result into an explicit unavailable `AccountAgeSeconds` fact while preserving `retryable=false`.
- Storage failures return retryable `Unavailable`. Tenant, actor and response contract failures remain validation, forbidden or invariant errors rather than degraded facts.
- `ServerForumPostingPolicyFactsComposer` composes the existing authoritative trust provider with the new account-age provider. Provider uniqueness remains enforced by `ForumPostingPolicyFactsComposer`.
- Host runtime composition publishes `Arc<ForumPostingPolicyFactsComposer>` alongside the existing `SharedForumAudienceFactsPort`; visibility and notification consumers retain their current audience facts runtime value.
- The published source-ready profile now contains authoritative trust and account age plus the existing local candidate link, mention and attachment counts.

## Ownership boundary

The server/users owner owns account creation time. Forum owns posting policy and consumes only a derived age in whole seconds through its typed owner-fact port.

`user.status`, email verification, sessions and login activity are not interpreted by this adapter. Authentication and account eligibility remain separate caller/owner responsibilities; account age is not a substitute for an active-account check.

`forum_user_stats` is not imported or read. Forum topic, reply and solution counters cannot define account age.

## Clock and degraded semantics

The adapter obtains one UTC observation timestamp after the exact user row is loaded and calculates `observation - created_at`. A future owner-enforcement slice may call the shared composer once per command and evaluate that returned snapshot; this slice does not create a distributed or persisted time reservation.

A missing user is not represented as zero seconds. When called through the composer it remains an explicit unavailable fact with stable owner code `forum.account_age_facts.user_not_found` and non-retryable classification. A storage outage remains retryable. A future timestamp is an invariant failure and is not hidden as an unavailable capability.

## Source proof

Inline SQLite/source tests cover:

- exact account-age seconds from a fixed observation time;
- combined authoritative trust and account-age composition without synthetic facts;
- missing exact user as non-retryable `NotFound`;
- future `created_at` as an invariant violation;
- foreign actor rejection before storage access.

The host extension test also records publication of the shared posting-policy composer without removing the existing audience facts capability.

## Excluded

- no reading or approved-post fact adapter;
- no active-flag or moderation-history fact adapter;
- no reputation ledger or fact adapter;
- no topic/reply/edit usage-window fact adapter;
- no bump-age fact adapter;
- no policy settings persistence or administration;
- no topic/reply/edit/bump owner enforcement;
- no shared distributed rate-limit reservation, commit, release or exact retry calculation;
- no duplicate-content hash or retained fingerprint;
- no external or AI scoring call;
- no trust promotion, demotion or other trust-state write;
- no migration, event, worker, GraphQL, REST, OpenAPI, admin UI or storefront change.

The next bounded FORUM-26 slice should add one Forum-owned activity fact without deriving it from aggregate `forum_user_stats`. A suitable boundary is an authoritative reading-activity snapshot based on the existing owner read-state model, with approved-post publication and distributed usage windows kept separate.

## Canonical plan debt

The canonical `crates/rustok-forum/docs/implementation-plan.md` is not replaced through the GitHub contents API. It exceeds two thousand lines and complete replacement risks unrelated roadmap loss. A safe repository-local edit still needs to mark FORUM-26 `in_progress`, record FORUM-26A-F and retain reading, approved posts, moderation, reputation, usage windows, enforcement, duplicate hashing, shared rate limiting and optional external scoring as remaining work.

`CRATE_API.md` is likewise not completely replaced. FORUM-26F introduces no new public Forum type; it implements and publishes the existing FORUM-26E contracts from the server host.

## Validation status

The following commands are source-ready but were not run by the implementation agent:

```text
cargo test -p rustok-server --features mod-forum forum_posting_policy_facts -- --nocapture
cargo test -p rustok-server --features mod-forum host_runtime_extensions_register_admin_mutation_providers -- --nocapture
cargo test -p rustok-forum --test posting_policy_facts -- --nocapture
node scripts/verify/verify-forum-account-age-posting-facts.mjs
node scripts/verify/verify-forum-posting-policy-facts.mjs
cargo xtask module validate forum
```

Tests, Cargo commands, formatting, verifier execution, workflows and CI remain the maintainer's responsibility for this slice.
