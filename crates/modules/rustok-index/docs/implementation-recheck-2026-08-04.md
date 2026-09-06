# `rustok-index` implementation recheck — 2026-08-04

Audited baseline: `main@9b7d72388400ec60dec52bf789cf523cb4024059`.

## Rechecked cursor

PR #2965 completed the database-neutral snapshot-pair digest producer and delegated unequal digests
to `PostgresIndexDriftFindingWriter`. The producer itself already accepted valid `EntityKey` values
with and without locale.

The next plan item correctly identified a remaining persistence mismatch: the typed finding scope,
writer adapter, inspector decoder, and original database CHECK required every entity finding to have
a locale. That caused an otherwise valid comparison for `LocaleMode::None` schemas to fail only at
persistence time.

## Scope correction

This slice keeps the existing locale-bearing scope and finding-key bytes unchanged and adds one
explicit `EntityWithoutLocale` scope. Locale absence is never represented as an empty or invented
`LocaleKey`.

The persisted key contract remains `index_drift_finding_key_v1`:

- locale-bearing entities retain the exact historical component sequence;
- locale-free entities append one length-prefixed NUL byte, which no canonical locale can contain.

The historical M3 migration remains unchanged. A new forward migration replaces only the findings
table scope CHECK so entity rows still require module, entity, positive schema version, and non-nil
entity UUID while `locale_key` becomes optional. Other digest, locale-shape, lifecycle, closure,
foreign-key, primary-key, and unique-key invariants remain.

## Source completed in this slice

- typed locale-free finding scope;
- writer validation, persistence, and deterministic key derivation;
- inspector decoding for `locale_key = NULL`;
- producer recorder mapping for both `EntityKey` locale shapes;
- forward PostgreSQL/SQLite migration registration;
- independent legacy-key compatibility contract;
- environment-gated real-migration PostgreSQL writer/inspector harness;
- current plan, architecture docs, and static verifier alignment.

## Deliberate limits

This recheck does not claim or add:

- a production `IndexDriftSnapshotReader`;
- PostgreSQL snapshot export or an owner high-watermark protocol;
- entity discovery, missing/stale enumeration, or orphan-link diagnosis;
- automatic convergence resolution;
- resolve/ignore commands, actor/reason audit, or public transport;
- targeted/full/shadow repair;
- retained execution evidence.

## Next cursor

Compose the first production snapshot reader for one exact entity under one truthful consistency
boundary. When owner state and Index state share PostgreSQL, prefer one transaction snapshot. When
they do not, require an explicit owner watermark contract and reject unprovable pairs.

## Verification ownership

The implementation agent did not run formatting, Cargo checks, tests, JavaScript verifiers,
PostgreSQL scenarios, workflows, or CI. The new and existing commands remain owner-run and are listed
in `implementation-plan-current-2026-08-03.md`.
