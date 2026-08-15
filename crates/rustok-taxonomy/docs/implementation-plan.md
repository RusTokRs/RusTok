# Implementation plan for `rustok-taxonomy`

## Current state

`rustok-taxonomy` owns tenant-scoped dictionary terms, translations, aliases,
canonical keys, and global/module scope rules. It is a vocabulary layer, not
shared product storage: blog, forum, product, and profiles retain their own
attachment tables and public domain contracts.

Term identity is locale-independent. Locale normalization and fallback use the
shared content contract. New consumers must attach terms through an explicit
owner-module relation table.

Taxonomy terms have no soft-deprecated/archive state. A persisted term is the
current canonical term identity. Removal is an actual delete; owner relation
foreign keys and route-key reservations must make deletion effects explicit
rather than hiding an unusable term behind a lifecycle flag. Historical
`archived` Taxonomy translation-change evidence is normalized to `active` by
the retained status-removal migration; real deletions remain `deleted`.

Localized route keys have one lookup namespace per
`tenant + kind + scope + locale`: a translation slug on one term cannot be
shadowed by an alias on another term, and vice versa. Module lookup prefers the
module scope over the global scope and follows requested locale -> explicit
fallback -> platform fallback. Owner transaction lookup uses the same existing
term identity and creates a new module term only when neither a route key nor a
canonical key resolves.

`taxonomy_term_route_keys` is the storage-level route ownership authority. Its
composite primary key is the complete localized route identity
(`tenant + kind + scope_type + scope_value + locale + route_key`) and `term_id`
is the owner. The migration preflights existing translation and alias rows
before creating the registry: same-term duplicate representations are
deduplicated, while a cross-term collision blocks migration with a deterministic
diagnostic instead of choosing a winner. Runtime localized mutation
finalization reconciles translation+alias rows with registry reservations in
the same transaction. Missing reservations are inserted before stale
reservations are released, so a concurrent claimant must win the single
database key or roll its mutation back. Term deletion removes reservations via
the composite tenant/term foreign-key cascade.

Service and consumer route resolution read `taxonomy_term_route_keys` directly.
The old translation-first/alias-second lookup path is not a second authority:
translation and alias tables hold localized content, while the registry owns
route identity and collision serialization.

Category hierarchy is deliberately outside the shared dictionary contract.
Parent/child category edges, ordering, cycle rules, and domain-specific category
metadata belong to the module that owns the category aggregate and its public
contract. Taxonomy must not acquire a generic `parent_id`, category tree, or
polymorphic relation table merely to centralize hierarchy. If a future domain
needs a shared hierarchical vocabulary, that requires a separate demonstrated
kind/ownership decision and explicit migration contract.

`TaxonomyTranslationTargetProvider` is registered by server composition as
`taxonomy/term`. It exposes exact source/target snapshots for `name`, `slug`,
and optional `description`; applies one target locale through resource/source/
target revision CAS; reports exact source/target progress; and reads the
append-only owner change cursor. Existing terms always expose
`TranslationResourceLifecycle::Active`; actual term deletion produces a
`deleted` change record. `slug` remains review-only for machine translation.
Provider apply uses the shared Outbox receipt ledger under owner slug
`taxonomy`, while Taxonomy retains authorization, validation, and the owner
transaction. The provider records durable owner change evidence but does not
claim a global `translation.target.changed` event contract.

The Profiles consumer now has demonstrated pressure for a per-profile locale
preference between requested locale and tenant default when presenting attached
Taxonomy tags. Taxonomy therefore exposes a tenant/kind-bounded owner read
projection for localized term names while Profiles keeps `profile_tags`, the
per-profile preference order, and batching semantics. This does not add a new
Taxonomy kind, generic relation storage, category hierarchy, or a second route
identity authority.

## FFA/FBA boundary

- FFA status: `not_started`
- FBA status: `boundary_ready`
- Structural shape: `no_ui_boundary`
- This dictionary module has no module-owned UI. Its owner-neutral Translation
  target SPI is an embedded capability boundary; no Taxonomy-specific UI or
  external transport is implied.

## Tracked results

Results 1-3 remain complete for the currently demonstrated contracts. Result 4
was previously complete, but the new Profiles-driven Taxonomy owner-read source
change is a semantic runtime-input change under the retained fingerprint policy.
Its exact-head PostgreSQL proof has been refreshed; post-merge main evidence is
still required before Result 4 may be called complete again. Future consumer
pressure, a genuinely new vocabulary kind, or a new operational incident class
must extend the tracked contract with explicit ownership and evidence rather
than weakening these established baselines.

1. **Keep dictionary and consumer contracts synchronized. — COMPLETE.** Update
   taxonomy terms, scope rules, consumer integrations, and manifest metadata
   atomically.
   **Depends on:** the change-owning consumer module.
   **Done when:** an owning module, rather than taxonomy, owns each attachment
   table and public relation contract.

   `scripts/verify/verify-taxonomy-ownership-boundary.mjs` is the continuous
   source guard for this boundary. It keeps Taxonomy production source free of
   generic category hierarchy and polymorphic consumer attachment storage, and
   requires the known Blog, Forum, Product, and Profiles relation tables to
   remain owner-module artifacts. Product category tree/closure storage plus
   Blog and Forum category hierarchy/translation storage are pinned to their
   owner modules. The lightweight `Taxonomy Ownership Boundary` workflow runs
   this guard whenever Taxonomy or one of those ownership-defining artifacts
   changes.

   `scripts/verify/verify-taxonomy-ownership-boundary-self-test.mjs` proves the
   guard fails closed on representative regressions: a Taxonomy `parent_id`, a
   consumer relation table moved into Taxonomy persistence, generic
   `owner_type/owner_id` attachment storage, missing Forum category translation
   ownership, missing Product category closure storage, and a missing owner-side
   profile relation artifact. The dedicated workflow runs this fixture suite
   before the real repository scan, and both checks are available through
   `scripts/verify/verify-all.sh` for local parity.

   `scripts/verify/verify-taxonomy-contract-matrix.mjs` closes the consumer
   metadata/public-contract side of the same ownership result. It requires Blog,
   Forum, Product, and Profiles to declare `taxonomy >=0.1.0` in their module
   manifests and to retain public documentation that names the module-owned
   relation (`blog_post_tags`, `forum_topic_tags`, `product_tags`, or
   `profile_tags`) while Taxonomy remains the shared vocabulary owner. The
   matrix also source-locks its own focused workflow triggers so a manifest or
   public relation-contract change cannot silently bypass the ownership gate.
   `verify-taxonomy-contract-matrix.test.mjs` proves manifest dependency drift,
   public relation-contract drift, and workflow-trigger drift fail closed.

   Production consumers are separately kept off Taxonomy SeaORM persistence by
   `scripts/verify/verify-taxonomy-persistence-boundary.mjs` and its negative
   fixture suite in the repository-wide `Hardening Gates` workflow. Owner-side
   relation reads/writes therefore remain with the consumer module while shared
   vocabulary access stays behind Taxonomy owner/service boundaries.

   Result 1 is complete for the current Blog/Forum/Product/Profiles consumer
   set after PR #3565. Exact-head run `31826897482` (`Taxonomy Ownership
   Boundary`), job `94853039929`, succeeded on
   `1a8d2b0fd5ac4183438e5890427b8238f0180853`, including the legacy ownership
   self-test/scan and the new contract-matrix self-test/real-repository scan.

2. **Expand kinds and lookup semantics only for demonstrated domain pressure. — COMPLETE.**
   Do not add speculative vocabulary kinds or polymorphic attachment storage.
   The current tag lookup baseline requires locale-aware route ownership,
   module-before-global precedence, shared locale fallback, registry-authority
   lookup, hard-delete-only term removal, and storage-level route-key
   reservation. Category parent/child hierarchy remains owner-domain state
   rather than an implicit taxonomy kind.
   **Depends on:** a concrete domain requirement and scope decision.
   **Done when:** canonical-key, alias/slug, tenant, module-scope, locale,
   deletion, and any newly demonstrated kind semantics are defined and tested.

   Owner-write batch semantics are exercised in
   `tests/localized_route_lookup.rs`: equivalent case/whitespace labels collapse
   to one normalized route identity, module scope wins before global even when
   the requested locale must use the platform fallback, and a global term is
   reused without creating a shadow module term when no module owner exists.
   Canonical-key fallback preserves the same module-before-global and tenant
   boundaries: a module canonical key wins before a global route, a global
   canonical key is reused without creating a shadow module term, and identical
   canonical keys in different tenants resolve only inside their own tenant.
   Module scope labels and locale tags are normalized at the Taxonomy boundary;
   owner modules may preserve their own input ordering/display casing without
   becoming a second identity authority.

   Hard deletion is also a route-identity lifecycle boundary. After a term is
   deleted, its localized route must stop resolving and its route, alias, and
   canonical identities must be available to a later replacement in the same
   tenant and scope. `tests/route_key_registry.rs` exercises the full
   resolve -> hard delete -> no result -> replacement reuse path in addition to
   the storage-level reservation cascade assertions.

   `.github/workflows/taxonomy-lookup-contract.yml` is the path-filtered Rust
   gate for these lookup, owner-write, and route-registry semantics. It runs
   both the localized lookup and route-registry integration suites whenever
   Taxonomy lookup, route-key, locale-normalization, migration, dependency-lock,
   kind DTO, or test inputs change, independently of unrelated workspace build
   jobs. `crates/rustok-taxonomy/src/dto.rs` is an explicit trigger, so adding a
   new `TaxonomyTermKind` cannot bypass the focused lookup suite.

   The contract matrix pins the currently demonstrated kind surface to exactly
   `Tag` and requires the established registry-authority, normalization,
   module/global precedence, locale fallback, canonical-key, tenant isolation,
   database route-ownership, and hard-delete-reuse regression cases to remain
   present. Its negative fixtures prove an unreviewed `Category` addition or
   removal of the `dto.rs` workflow trigger fails closed. This is not a claim
   that Taxonomy can never gain another kind: a future kind must arrive with a
   concrete domain requirement, explicit ownership decision, lookup/lifecycle
   semantics, migrations where required, and dedicated executable evidence.

   Result 2 is complete for the demonstrated Tag baseline after PR #3565.
   Exact-head run `31826897506` (`Taxonomy Lookup Contract`), job
   `94853039807`, succeeded on
   `1a8d2b0fd5ac4183438e5890427b8238f0180853`: the route-registry recovery
   diagnostic passed and both `localized_route_lookup` and
   `route_key_registry` integration binaries completed successfully.

3. **Maintain dictionary operational guidance. — COMPLETE.** Add documentation
   and runbooks when a changed vocabulary contract introduces drift or
   integration recovery risk. Route-registry migration failures must be repaired
   by resolving the reported cross-term owner collision, never by deleting an
   arbitrary winner.
   **Depends on:** an actual runtime or consumer incident class.
   **Done when:** operators can reconcile terms, aliases, registry reservations,
   and owner attachments without inventing shared relation ownership.

   `docs/route-registry-recovery.md` is the retained recovery procedure for the
   concrete registry-drift incident class exposed by registry-authority lookup.
   Operators diagnose the complete
   `tenant + kind + scope_type + scope_value + locale + route_key` tuple with
   read-only queries, then repair missing/stale reservations through the normal
   Taxonomy owner/service mutation path. Direct production writes to
   `taxonomy_term_route_keys` are explicitly forbidden: localized mutation
   finalization reconciles desired reservations before durable change evidence
   commits, and a conflicting registry owner must make the repair fail closed.

   The runbook also keeps attachment repair with Blog (`blog_post_tags`), Forum
   (`forum_topic_tags`), Product (`product_tags`), and Profiles (`profile_tags`),
   and keeps Blog/Forum/Product category hierarchy in those owner modules.
   `tests/route_key_registry.rs` proves all three concrete recovery outcomes: a
   normal owner-service update restores a deliberately missing reservation; an
   unchanged authoritative re-save releases a deliberately stale registry-only
   route while preserving desired slug/alias ownership and lookup; and a
   cross-term collision rejects the losing repair without stealing the existing
   registry owner. No generic Taxonomy relation table or `parent_id` is part of
   recovery.

   Result 3 is complete for the demonstrated route-registry drift incident
   class after PRs #3549, #3550, and #3559. Exact-head PR #3559 run
   `31818118097` (`Taxonomy Lookup Contract`) and run `31818118039`
   (`Taxonomy Ownership Boundary`) succeeded on
   `5912fb172006b707b083260c0636bc8c6ea945f5`; PostgreSQL evidence run
   `31818118093` also succeeded on that same head. A future distinct incident
   class can add new guidance without reopening or weakening the established
   missing/stale/cross-term recovery contract.

4. **Collect production target and route-registry evidence. — REFRESH PENDING POST-MERGE MAIN EVIDENCE.** Run
   the canonical server migration graph, including the owner-operation receipt
   dependency and retained Taxonomy migrations, plus PostgreSQL concurrent
   localized-write, translation apply, and change-cursor scenarios before
   treating the route registry and `taxonomy/term` target as production-proven
   under replicas.
   **Depends on:** a production-like PostgreSQL runtime.
   **Done when:** retained migration/backfill, two-writer route-key contention,
   translation apply CAS, and cursor-recovery evidence prove that exactly one
   route owner can commit and the registered provider remains correct under
   multi-replica conditions, with both an exact-head pull-request run and a
   post-merge main run over the same fingerprinted runtime inputs.

   Source evidence for the two-writer route-key contention portion is executable
   in `tests/route_registry_contention_postgres.rs`. With
   `RUSTOK_TAXONOMY_TEST_DATABASE_URL` it creates an isolated PostgreSQL schema,
   runs the retained Taxonomy migrations, uses two independent writer
   connections, forces both localized writers past route preflight before
   releasing their translation-row locks, and verifies that the registry key
   admits exactly one commit while the losing translation rolls back.

   Source evidence for translation-target CAS and cursor recovery is executable
   in `tests/translation_target_postgres.rs`. It requires the canonical
   PostgreSQL schema produced by the canonical server Migrator and refuses to
   run if the owner-operation receipt ledger or required Taxonomy tables are
   absent. The scenarios use unique tenant identities and independent
   single-session connections, then race two applies from the same exact
   source/target snapshot and expected revisions. Exactly one stale-revision
   candidate may commit; the loser must close as a conflict, leaving one
   resource/target revision advance and one durable winning change fact. A
   separate recovery scenario resumes from the create cursor after provider
   reconstruction, applies an exact target, reconstructs again around hard
   deletion, resumes the `deleted` lifecycle change, drains after the latest
   cursor, and verifies progress exposes that latest durable owner cursor.
   Sequential recovery writes are separated across ULID milliseconds for
   deterministic cursor ordering; this does not claim an arbitrary concurrent
   transaction commit-order guarantee.

   `.github/workflows/taxonomy-postgres-evidence.yml` is the retained runtime
   path. It provisions PostgreSQL 16, checks out the exact pull-request head (or
   exact push SHA), activates and asserts Rust `1.96.0`, runs `rustok-migrate up`
   against the ephemeral database, executes both Taxonomy PostgreSQL harnesses,
   archives migration/test provenance and logs, and requires the source/runtime
   gate. During an evidence refresh, the runtime job may still collect proof
   after a deliberately stale source snapshot fails; the final gate remains
   fail-closed until the snapshot itself is updated.

   The previous Result 4 completion remains historical evidence: exact-head run
   `31738994542` and post-merge main run `31745429243` proved the earlier
   fingerprint set. The Profiles-driven owner-read source change invalidated
   that set exactly as the source verifiers are designed to do; those old runs
   are not reused as proof for the current runtime inputs.

   Fresh exact-head refresh run `31845977594` exercised
   `0118ddd0dc73edceefb01eb2e82e29f03a4dc228` on PostgreSQL 16 with an asserted
   Rust `1.96.0` toolchain. Runtime job `94912907567` successfully applied the
   canonical server migrations, passed the two-writer route-key contention
   harness, and passed both translation-target CAS/cursor scenarios. Artifact
   `9236159727`, named
   `taxonomy-postgres-evidence-31845977594-0118ddd0dc73edceefb01eb2e82e29f03a4dc228`,
   records digest
   `sha256:6c42063e312f9113eb0cc3d014c28b227d4ae952440e4b52085b4ad8e46117a2`.
   Its metadata records identical expected/actual head SHAs and the active
   `1.96.0-x86_64-unknown-linux-gnu` directory override. The refresh run's
   source job `94912365639` and gate `94914729792` failed by design against the
   stale pre-refresh snapshot; the runtime job itself succeeded and produced the
   replacement exact-head evidence.

   Result 4 refresh is pending post-merge main evidence. The evidence contracts
   therefore carry exactly one open Result 4 item,
   `post_merge_main_postgresql_evidence`. After this change reaches `main`, a
   fresh exact-main PostgreSQL run must succeed and be recorded in a separate
   evidence-only change before Result 4 returns to `COMPLETE` and the open item
   is removed.

   Both source verifiers compare recorded runtime input fingerprints with the
   current Git blob/tree identities, so any later semantic runtime-input change
   invalidates the recorded evidence and requires another fresh PostgreSQL run.
   This evidence remains production-like PostgreSQL 16 CI evidence; it does not
   claim observation of live production traffic or arbitrary concurrent
   transaction commit ordering.

## Verification

- `cargo xtask module validate taxonomy`
- `cargo xtask module test taxonomy`
- `node scripts/verify/verify-taxonomy-ownership-boundary-self-test.mjs`
- `node scripts/verify/verify-taxonomy-ownership-boundary.mjs`
- `node scripts/verify/verify-taxonomy-contract-matrix.test.mjs`
- `node scripts/verify/verify-taxonomy-contract-matrix.mjs`
- `node scripts/verify/verify-taxonomy-persistence-boundary.test.mjs`
- `node scripts/verify/verify-taxonomy-persistence-boundary.mjs`
- `cargo test -p rustok-taxonomy --test localized_route_lookup`
- `cargo test -p rustok-taxonomy --test route_key_registry`
- Targeted term CRUD, cross slug/alias collision, scope restriction, locale
  fallback, owner-write batch identity, canonical-key tenant isolation,
  registry-authority lookup, hard-delete route lookup/reuse, route-registry
  reservation/release/cascade/recovery, status-removal migration, and
  consumer-integration tests.
- `cargo test -p rustok-taxonomy --lib`
- `DATABASE_URL=postgresql://... cargo run --locked -p rustok-migrations --bin rustok-migrate -- up`
- `RUSTOK_TAXONOMY_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-taxonomy --test route_registry_contention_postgres -- --nocapture`
- `RUSTOK_TAXONOMY_TEST_DATABASE_URL=postgresql://... cargo test -p rustok-taxonomy --test translation_target_postgres -- --nocapture`
- Recorded PostgreSQL runtime provenance remains guarded by both Taxonomy
  evidence verifiers and runtime input fingerprints; future semantic changes
  must produce fresh evidence rather than silently reusing recorded runs.
- While Result 4 refresh is pending, the exact-head proof and the single open
  post-merge evidence item must remain explicit; neither source verifier may
  silently promote the result to complete.

## Change rules

1. Keep dictionary terms and scope policy in this module.
2. Keep parent/child category hierarchy, ordering, cycle validation, and
   domain-specific category metadata with the owning module unless an explicit
   shared-hierarchy ADR changes ownership.
3. Keep localized route mutations and `taxonomy_term_route_keys` reservations
   in one database transaction; never repair collisions by table-order
   precedence or backend-specific trigger behavior.
4. Do not add a soft-deprecated term state. A term either exists as the current
   identity or is deleted through the explicit owner/storage contract.
5. Update local docs, `rustok-module.toml`, and consumer docs with a taxonomy
   contract change.
6. Update `docs/modules/registry.md` with any ownership or module-status change.
