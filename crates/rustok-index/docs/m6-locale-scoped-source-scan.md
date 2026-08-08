# M6 locale-scoped replay source scan foundation

Status: `generic_source_contract_complete_product_source_pending`.

## Purpose

Replay checkpoints already reserve a `locale_key`, but the generic source scan request was schema-wide. Populating a checkpoint locale without a real locale-aware source request would create false isolation: a supposedly locale-scoped replay could still scan and mutate every locale in the schema.

This slice establishes the generic source boundary first. Durable replay job/checkpoint locale identity, Product PostgreSQL locale predicates, GraphQL locale input, partition scope and rebuild modes are intentionally separate follow-up work.

## Generic source request

`IndexSourceScanRequest` now carries `locale: Option<LocaleKey>`.

The existing `IndexSourceScanRequest::new(...)` constructor is unchanged in meaning and constructs a schema-wide scan with no locale scope. Existing source implementations therefore keep their current call shape and behavior.

`IndexSourceScanRequest::for_locale(...)` constructs an exact-locale request from an already canonical `LocaleKey`. The request exposes that scope through `locale()`.

Schema `LocaleMode` admission is deliberately not guessed inside the request constructor. The later replay worker layer owns admission because it has the registered `IndexSchema` contract available. A locale replay against `LocaleMode::None` must fail closed there rather than silently becoming schema-wide.

## Fail-closed page validation

`IndexSourcePage::new` and `SharedIndexSourceRegistry::scan` continue to validate every returned page through the common source contract.

For a locale-scoped request, every returned mutation key must contain exactly the requested locale. A source that ignores the locale request and returns another locale fails with `IndexSourceError::ScanMutationLocaleMismatch`.

This validator is a safety net, not a pagination strategy. A real locale-capable source must constrain its underlying scan before pagination; post-filtering a schema-wide page would be incorrect because it could skip matching rows or return empty continuation pages.

## Canonical locale identity

The request accepts `LocaleKey`, not a raw string. The retained source fixture uses a BCP-47-compatible mixed-case locale (`EN-us`) and requires canonical `en-US`, so casing/alias normalization cannot create separate replay scan identities.

## Explicit non-goals in this slice

This slice does not:

- change `IndexReplayPageRequest`, `IndexReplayRunRequest`, replay jobs, leases or checkpoints;
- write non-empty `index_jobs.locale_key` or `index_checkpoints.locale_key`;
- add or change a database migration;
- add Product SQL locale filtering yet;
- add GraphQL locale input;
- add `partition_key` behavior;
- introduce targeted/full/shadow rebuild modes;
- change M7 Storefront serving behavior.

The next source slice should make one real `LocaleMode::Required` source — current Product — honor `request.locale()` in SQL before pagination while preserving its current schema-wide scan when locale is absent. Only after that source capability exists should durable replay job/checkpoint locale identity be admitted.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or `git diff --check` were executed by the implementation agent.
