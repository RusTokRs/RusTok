# FORUM-24R Search canonical-route reindex harness

Status: **executable PostgreSQL source / maintainer execution pending**

## Scope

FORUM-24Q replaced Forum Search UUID query navigation with localized routes projected by the Forum route owners. Existing Search rows still require a complete Forum projection rebuild before runtime promotion.

FORUM-24R adds an executable PostgreSQL harness for that handoff without changing production runtime behavior:

```text
crates/rustok-search/tests/forum_canonical_route_reindex_postgres.rs
```

The machine-readable evidence contract is:

```text
crates/rustok-forum/contracts/forum-search-canonical-route-reindex-harness.json
```

## Real composition

The harness creates a unique PostgreSQL schema and applies the actual prerequisite and owner migrations:

- Outbox;
- Taxonomy;
- the shared Flex field-definition cache-generation prerequisite;
- Forum;
- Search.

It creates a public category, topic, and approved reply through `CategoryService`, `TopicService`, and `ReplyService`. The production `ForumSearchProjectionSourceFactory` is then composed into `SearchIngestionHandler::with_forum_source`.

No fake projection source, copied route builder, direct `ForumSearchProjector` construction, or transport-local compatibility URL is used.

## Reindex path

The harness sends the existing event:

```text
ReindexRequested { target_type: "forum", target_id: None }
```

The event enters the durable Forum inbox and is applied through the existing Search ingestion owner. `ForumSearchProjector` creates its temporary stage, reads bounded pages from the Forum projection, deletes only the selected tenant Forum scope, inserts the staged rows, and commits the replacement transaction.

The harness does not call the stage or scope-delete implementation directly.

## Evidence cases

Before reindex, the selected tenant contains:

- a legacy category route;
- a legacy topic route;
- a legacy reply route;
- a stale orphan Forum topic document.

A second tenant contains its own legacy Forum document.

After reindex, the harness asserts:

- exactly one current category, topic, and approved-reply document exists for the selected tenant;
- category route is `/en/forum/c/platform`;
- topic route is `/en/forum/t/{short_id}/canonical-search`;
- reply route is the same canonical topic route plus `reply={reply_id}`;
- `canonical_search_result_url` accepts each owner-projected route;
- no `/modules/forum` route remains in the rebuilt tenant;
- the stale orphan is absent;
- the other tenant document remains unchanged.

This proves tenant-scoped staged replacement and route cutover together. It does not claim deployment reindex completion.

## Preserved boundaries

FORUM-24R adds no production runtime code, migration, route owner, storage schema, event schema, visibility policy, GraphQL/native DTO, or compatibility fallback.

The harness intentionally clears fixture author identities after owner creation so the route/reindex proof does not require a Profiles fixture. Author projection behavior remains covered by its existing contracts and is not reimplemented here.

## Execution handoff

The test uses:

```text
RUSTOK_SEARCH_TEST_DATABASE_URL
```

and falls back to `DATABASE_URL`. When neither contains a PostgreSQL URL, it returns successfully after reporting that the environment-gated scenario was skipped.

Maintainers can run:

```bash
node scripts/verify/verify-forum-search-canonical-route-reindex-harness.mjs
cargo test -p rustok-search --test forum_canonical_route_reindex_postgres -- --nocapture
```

FORUM-24S separately adds registered native-host SQLite evidence for the canonical and historical category/topic route decisions:

```text
crates/rustok-forum/storefront/tests/native_host_route_decision_sqlite.rs
```

That target confirms the route decisions are reachable through the real Axum/Leptos server-function dispatcher, but it also remains unexecuted.

Runtime promotion still requires running a complete Forum Search reindex in the target environment and checking GraphQL, native storefront, Search admin, and admin-shell navigation.

## Validation status

No tests, Node verifiers, Cargo commands, formatting, PostgreSQL or SQLite scenarios, reindex operations, workflows, HTTP requests, browser scenarios, or CI were run while preparing these evidence slices.

## Roadmap note

`crates/rustok-forum/docs/implementation-plan.md` remains the only authoritative Forum roadmap. Its FORUM-24 ledger is stale relative to the merged source slices. These bounded evidence documents do not create a second roadmap or claim canonical ledger synchronization.
