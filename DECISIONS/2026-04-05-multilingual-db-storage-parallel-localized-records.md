# Multilingual DB storage via parallel localized records

- Date: 2026-04-05
- Status: Accepted

## Context

RusToK already uses a strong runtime and UI i18n contract, but database storage is still mixed:

- some modules keep locale columns too narrow for the current BCP47-like runtime contract;
- some entities still rely on localized fields in base rows or mixed JSON payloads;
- `flex` is a capability/ghost module, but it still needs to follow the same multilingual storage rules instead of becoming a storage exception.

The platform also needs to support frequent tenant default-locale changes without rewriting data or treating one language as physically privileged.

## Decision

RusToK adopts a single multilingual storage target for platform and module data:

- base business rows store only language-agnostic state;
- short localized text lives in parallel `*_translations` records;
- heavy localized content may live in parallel `*_bodies` records when the content shape or size justifies a dedicated table;
- tenant locale policy (`tenants.default_locale`, `tenant_locales`) influences only effective locale selection and fallback, never physical ownership of localized fields;
- locale storage must support the same normalized BCP47-like contract as runtime, with a platform-safe column width of `VARCHAR(32)`.

`flex` is explicitly included in this decision:

- attached-mode `flex` does not own donor entities, but any canonical multilingual data introduced by `flex` must still use parallel localized records instead of donor base-row blobs;
- standalone `flex` must follow the same pattern: base schema/entry rows plus parallel localized rows for localized schema copy and, in later slices, localized entry content where field semantics require it.

This ADR defines the target state. Migration can proceed in slices, but new work must move toward this model rather than introducing fresh exceptions.

## Consequences

- foundation locale columns must be widened to `VARCHAR(32)` and documented as part of the live contract;
- module storage docs must describe base-vs-translation ownership more strictly;
- `flex` standalone storage must stop treating schema-localized copy as inline base-row data;
- attached-mode `flex` field-definition localization stored as JSON locale maps is now considered transitional, not the final DB shape;
- some legacy mixed patterns may remain temporarily during rollout, but they are legacy exceptions that need explicit documentation and follow-up migration work.
