# FORUM-15A profile presentation fallback actualization — 2026-08-10

Status: `source-ready / fallback-fail-closed / stats-enrichment-open / maintainer-execution-open`

## Fresh cursor

This slice started from `main@535c6d4a3aca4412c27c69453e08c6942c281ac9` after FORUM-14A and the later Pages/Commerce main movement.

FORUM-14 attachment persistence remains blocked. Media owns authoritative asset lifecycle state, but the current public Media presentation/read contract still does not publish the lifecycle eligibility required by the Forum ownership plan, and the current Media lifecycle enum has no quarantine state to consume. This slice therefore does not weaken FORUM-14A by treating `get_asset` existence as attachment persistence admission.

The next independently actionable Forum cursor is FORUM-15. The canonical ledger remains `in_progress`: Profiles already owns presentation/privacy behavior, while Forum still needs complete member-card composition, Forum-stat enrichment and retained no-N+1 evidence.

## Existing Profiles owner boundary rechecked

`ProfilePresentationService` is the downstream presentation owner. Its bounded summary path:

1. evaluates the canonical profile privacy/block matrix with `ProfilePrivacyService::evaluate_access_batch`;
2. keeps only `ProfilePrivacyDecision::Allow` identities;
3. loads localized summaries only for the allowed identities;
4. maps privacy-owner failures to `PresentationUnavailable` instead of returning raw profile data.

`ProfileSummaryLoader` is likewise presentation-aware. It groups requests by tenant and locale context and delegates each group to `ProfilePresentationService::for_audience`.

The server request extension binds that loader to the actual request audience:

- anonymous request -> `ProfileAccessAudience::Anonymous`;
- human request -> `ProfileAccessAudience::Authenticated { actor_id }`;
- service principal -> `ProfileAccessAudience::TrustedService { actor_id: None }`.

Forum must consume these public presentation contracts and must not reproduce Profiles/Social Graph private-table policy.

## Forum GraphQL gap

Forum topic/reply GraphQL already avoids the ordinary profile N+1 pattern. `load_author_profiles_map` deduplicates author UUIDs and prefers the request-scoped `DataLoader<ProfileSummaryLoader>` for one batched owner presentation read.

The unsafe edge was the fallback used when a host did not install that request-scoped loader. It constructed raw `ProfileService` and called the `ProfilesReader` summary method directly. Raw `ProfileService` is an owner-internal reader and does not carry the request audience presentation policy, so an alternative host/test composition could expose a profile summary that the presentation owner would have hidden.

## Fail-closed fallback

FORUM-15A keeps the request-scoped loader as the preferred path and changes only the missing-loader fallback to:

```rust
ProfilePresentationService::new(db.clone())
    .find_profile_summaries(...)
```

`ProfilePresentationService::new` is intentionally anonymous/fail-closed. If a host fails to compose the request-aware loader, Forum no longer silently upgrades to raw profile visibility. Authenticated-only, follower-only, private or blocked presentation remains unavailable unless the proper audience-aware host composition is present.

This is deliberately least-privilege fallback behavior; it is not a substitute for the authenticated request loader.

## Scope preserved

The existing author lookup shape remains bounded:

- author IDs are deduplicated before owner access;
- the normal DataLoader path remains unchanged;
- the fallback still performs one batched presentation call for the deduplicated IDs;
- Forum does not query Profiles or Social Graph tables;
- Forum does not copy handle/display-name/avatar/privacy state into Forum persistence;
- no mutation, migration, event, queue, receipt or new dependency is introduced.

The change applies to the shared helper used by Forum topic lists, selected topics and reply lists, including storefront variants.

## Remaining FORUM-15 work

This slice does not claim complete member cards.

The next bounded source slice should compose Forum-owned statistics without introducing a per-author Forum query. A suitable direction is one deduplicated bounded Forum user-stat read for the same visible author set, then a presentation DTO that combines the Profiles-owned summary with Forum-owned topic/reply/solution counts. The profile presentation decision must remain authoritative: hidden profiles must not become visible merely because Forum statistics exist.

Runtime/browser evidence and explicit no-N+1 execution evidence also remain maintainer work.

## Canonical plan status

The canonical FORUM-15 ledger remains correctly `in_progress`; its broad remaining-work statement is still true after this narrow fallback hardening, so this slice does not rewrite the large canonical plan merely to record a sub-slice.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-profile-presentation-fallback-source.mjs
```
