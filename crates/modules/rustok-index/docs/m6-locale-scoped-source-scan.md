# M6 locale-scoped replay source scan foundation

Status: `product_source_complete_durable_replay_scope_pending`.

## Purpose

Replay checkpoints already reserve a `locale_key`, but the original generic source scan request was schema-wide. Populating a checkpoint locale without a real locale-aware source request would create false isolation: a supposedly locale-scoped replay could still scan and mutate every locale in the schema.

The generic exact-locale request and one current `LocaleMode::Required` source — Product — now carry that scope end-to-end through the source boundary. Durable replay job/checkpoint locale identity, GraphQL locale input, partition scope and rebuild modes remain separate follow-up work.

## Generic source request

`IndexSourceScanRequest` carries `locale: Option<LocaleKey>`.

The existing `IndexSourceScanRequest::new(...)` constructor is unchanged in meaning and constructs a schema-wide scan with no locale scope. Existing source implementations therefore keep their current call shape and behavior.

`IndexSourceScanRequest::for_locale(...)` constructs an exact-locale request from an already canonical `LocaleKey`. The request exposes that scope through `locale()`.

Schema `LocaleMode` admission is deliberately not guessed inside the request constructor. The later replay worker layer owns admission because it has the registered `IndexSchema` contract available. A locale replay against `LocaleMode::None` must fail closed there rather than silently becoming schema-wide.

## Fail-closed page validation

`IndexSourcePage::new` and `SharedIndexSourceRegistry::scan` continue to validate every returned page through the common source contract.

For a locale-scoped request, every returned mutation key must contain exactly the requested locale. A source that ignores the locale request and returns another locale fails with `IndexSourceError::ScanMutationLocaleMismatch`.

This validator is a safety net, not a pagination strategy. A real locale-capable source must constrain its underlying scan before pagination; post-filtering a schema-wide page would be incorrect because it could skip matching rows or return empty continuation pages.

## Product PostgreSQL source

Current Product remains one physical Index entity per Product translation locale and keeps its current schema-wide replay path intact when `request.locale()` is absent:

- first page orders by `(product_id, locale)`;
- continuation uses `(row.product_id, row.locale) > ($2, $3)`;
- the existing `ProductCursor { product_id, locale }` wire format is unchanged.

For `request.locale() = Some(locale)`, Product now constrains the underlying PostgreSQL query before pagination:

- first page uses `WHERE row.locale = $2`, orders by `row.product_id`, and limits with `$3`;
- continuation uses `WHERE row.locale = $2 AND row.product_id > $3`, orders by `row.product_id`, and limits with `$4`;
- the requested canonical locale is bound as a SQL parameter rather than interpolated;
- a supplied Product cursor must contain exactly the same canonical locale or the source fails with `product_index_cursor_invalid` before querying storage.

The Product cursor deliberately keeps its locale field even on locale-scoped scans. That preserves one stable cursor wire contract and makes cross-locale cursor reuse explicitly detectable rather than silently reinterpreted.

After row decode, the generic source-page validator supplies a second fail-closed boundary: every emitted Product mutation must still carry exactly the requested `LocaleKey`.

## Canonical locale identity

The request accepts `LocaleKey`, not a raw string. The retained generic source fixture uses a BCP-47-compatible mixed-case locale (`EN-us`) and requires canonical `en-US`, so casing normalization cannot create separate replay scan identities.

Product cursors already require their serialized locale to be canonical. Locale-scoped replay adds the stronger requirement that the cursor locale equals the request locale.

## Explicit non-goals in this slice

This slice does not:

- change `IndexReplayPageRequest`, `IndexReplayRunRequest`, replay jobs, leases or checkpoints;
- write non-empty `index_jobs.locale_key` or `index_checkpoints.locale_key`;
- add or change a database migration;
- add GraphQL locale input;
- add `partition_key` behavior;
- introduce targeted/full/shadow rebuild modes;
- change M7 Storefront serving behavior.

The next source slice can now introduce honest durable locale replay identity: worker `LocaleMode` admission, a locale replay job scope, locale-bearing lease/request identity and matching `index_checkpoints.locale_key`. It must preserve the existing schema-wide replay contract and leave `partition_key` empty until a real partition-scoped source contract exists.

No Rust tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or `git diff --check` were executed by the implementation agent.
