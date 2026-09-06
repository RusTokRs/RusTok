# FORUM-23B2G2B3D14 translation and moderation Search proof

## Status

`source_ready_maintainer_execution_pending`

This slice adds an executable PostgreSQL proof for two remaining
`LINK-FORUM-03` behaviors that already have real Forum owner commands:

- locale-specific category and topic translation projection;
- pending reply approval into public Search visibility.

It does not execute or simulate topic move. Topic move remains a planned owner
workflow under `FORUM-21`, so direct SQL mutation would not be valid cross-module
evidence.

The machine contract is:

```text
crates/rustok-forum/contracts/forum-search-link-forum-03-translation-moderation-proof.json
```

The executable test is:

```text
apps/server/tests/forum_versioned_invalidation_translation_moderation.rs
```

Successful execution writes:

```text
target/forum-search-link-forum-03-translation-moderation-evidence.json
```

## Runtime topology

The test creates one isolated PostgreSQL schema, creates only the minimal
`users` owner table required by Forum foreign keys, and applies the real
migrations from:

- `OutboxModule`;
- `TaxonomyModule`;
- `ForumModule`;
- `SearchModule`.

It uses production Forum owner services, the production
`ForumSearchContractIngress`, the production Forum projection source, the
production `ForumProjectionReconciler`, the shared Forum storefront Search
execution, and the real Forum result-eligibility owner.

No fake projection source, direct Search document write, manual typed-event
construction or alternate inbox is used.

## Exact owner revision trace

The fixture contains one moderated category, one public English topic and one
pending English reply.

The proof requires the following contiguous Forum owner revisions:

1. category creation: target `forum`, null target ID;
2. topic creation: target `forum_category`, exact category ID;
3. French category translation update: target `forum`, null target ID;
4. French topic translation update: target `forum_topic`, exact topic ID;
5. pending-to-approved reply moderation: target `forum_category`, exact category
   ID.

Creating the pending reply must not allocate a public projection revision. That
is part of the proof: pending content remains owner state but does not enter the
public Search projection until the real moderation command changes it to
`approved`.

For every revision the test reads the exact legacy root and caused typed envelope
from `sys_events`. It requires:

- ledger event ID equals legacy root envelope ID;
- typed envelope ID is distinct;
- typed `causation_id` equals the root/ledger ID;
- typed owner revision, target type and target ID equal the ledger row;
- registered event schemas remain valid.

The exact typed envelopes enter the existing Search inbox through
`ForumSearchContractIngress`.

## Translation phase

The test first projects the English category and topic and verifies that the
pending reply is absent from Search and storefront results.

It then calls:

- `CategoryService::update` with locale `fr`;
- `TopicService::update` with locale `fr`.

The category translation is created first because a public French topic document
requires a public French category translation. After the two exact typed
invalidations are durably admitted and reconciled, the proof requires:

- English category and topic documents remain present;
- French category and topic documents exist with exact marker text;
- the French topic is returned through the production storefront path for locale
  `fr`;
- the English topic remains returned for locale `en`;
- the pending reply remains absent.

## Moderation phase

The test calls real `ModerationService::approve_reply` for the pending reply. The
owner transaction must persist:

- the pending-to-approved reply state transition;
- the legacy `forum.reply.status_changed` event;
- the category projection owner revision and its root/typed invalidation;
- public counters maintained by the owner workflow.

The exact legacy moderation event is inserted into the existing
`search_projection_inbox`, followed by the exact caused typed invalidation through
`ForumSearchContractIngress`. Their `ingest_sequence` values must increase in
that delivery order. Forum `owner_revision` is an independent causal clock and is
never compared numerically with Search `ingest_sequence`.

After the production reconciler processes both events, the proof requires one
approved reply document with the exact body, topic and category identities and
one exact storefront result. English and French topic documents must remain
visible.

## Idempotence and retention

Every owner root ID and the moderation status-event ID must retain exactly one
Search inbox row. A caught-up second reconciler sweep must claim and complete no
work.

The JSON evidence is written only after:

1. all owner, envelope, inbox, projection and storefront assertions pass;
2. the isolated PostgreSQL schema is removed;
3. `git rev-parse HEAD` returns one valid 40-character source commit.

Skipped tests do not write acceptable evidence.

## Maintainer execution

```bash
node scripts/verify/verify-forum-search-link-forum-03-translation-moderation-proof.mjs
RUSTOK_SEARCH_TEST_DATABASE_URL="$DATABASE_URL" cargo test -p rustok-server --test forum_versioned_invalidation_translation_moderation -- --nocapture --test-threads=1
```

## Remaining LINK-FORUM-03 scope

This proof does not close `LINK-FORUM-03`. The remaining executable scope still
includes:

- a real topic move and category-scope projection update after `FORUM-21`
  delivers the owner command;
- exact private and trusted-channel exclusion runtime evidence;
- final retained assembly and review with the D13 core artifact.

D14 changes no production Rust path, migration, event schema or digest, DTO,
runtime flag, dependency, `Cargo.toml` or `Cargo.lock` entry.

No command above was run by the implementation agent, per maintainer request.
