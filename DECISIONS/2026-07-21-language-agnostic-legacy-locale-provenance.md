# Truthful locale provenance for legacy localized rows

- Date: 2026-07-21
- Status: Accepted
- Extends: `2026-04-05-multilingual-db-storage-parallel-localized-records.md`

## Context

RusToK requires language-agnostic base rows and parallel localized records. Several historical cutover migrations correctly moved localized copy out of base tables, but assigned the platform fallback locale `en` to text whose original language was not recorded.

A platform fallback is a runtime selection policy. It is not evidence about the language of stored business copy. Assigning `en` during a storage migration fabricates provenance, physically privileges one language, and can make later tenant default-locale changes reinterpret old data incorrectly.

## Decision

Legacy localized text must preserve truthful locale provenance:

- when the original locale is known from an authoritative row or immutable command/event context, store that normalized locale;
- when a provenance-bearing history/event table permits an absent locale, store `locale = NULL` together with explicit legacy/unknown provenance;
- when an ordinary translation table requires a non-null locale and the original locale is unknown, store the normalized BCP47 language tag `und` (`undetermined`);
- `und` is storage-only provenance. It must not be inserted into `tenant_locales`, selected as an effective request locale, or used as the platform fallback;
- runtime reads must not silently substitute `und` for a requested locale. Operators or explicit migration tooling must relocalize the row into a real locale;
- migrations must never bind unknown text to `PLATFORM_FALLBACK_LOCALE`, a tenant's current default locale, or a hardcoded language;
- locale columns continue to use the platform-safe `VARCHAR(32)` contract;
- widening locale columns is forward-only. Rollback must not narrow them and risk truncating valid normalized tags.

## Consequences

- historical cutover source that inserts unknown copy as `en` must be corrected to use `und` before clean installs are promoted;
- already-applied databases cannot safely rewrite every `en` row automatically because a row may have been edited later as genuine English copy; those databases require an explicit operator audit;
- source verification must reject narrow live locale columns and hardcoded fallback-language attribution in storage migrations;
- language-agnostic base rows must not retain a second mutable copy after localized/event history becomes authoritative.
