# Public API policy for `rustok-ui-i18n`

`rustok-ui-i18n` is still at workspace version `0.1.0`, but its public Rust surface is already consumed broadly enough that accidental removals should be treated as compatibility changes rather than ordinary cleanup.

This document classifies the current surface before 1.0. It does **not** remove any existing export.

## Preferred high-level surface

New module-owned UI code should prefer `rustok_ui_i18n::prelude::*` when a glob import is appropriate. The prelude intentionally contains only the consumer-facing runtime and macro layer:

- `UiMessages`
- `PreparedUiMessages`
- `UiTranslator`
- `UiLocaleTranslator`
- `BundleBuildError`
- `I18nError`
- `FluentArgs`
- `declare_module_i18n!`
- `fluent_args!`
- `t!`
- `module_t!`

`FluentArgs` is retained in this tier because argument-bearing public formatting methods accept it directly and the public macros construct it.

The prelude is additive. Existing crate-root imports remain valid.

## Supported lower-level interoperability surface

The following APIs are intentionally available for callers that need catalog construction, explicit locale utilities, diagnostics, or direct resolution rather than the module facade:

- `build_fluent_bundle`
- `build_fluent_catalog`
- `try_build_fluent_catalog`
- `bundle::build_fluent_catalog_report`
- `bundle::FluentCatalogBuildReport`
- `FluentCatalog`
- `normalize_admin_locale`
- `normalize_locale_tag`
- `locale_candidates`
- `resolve_fluent_message`
- `try_resolve_fluent_message`
- `with_kebab_key`

These APIs are deliberately omitted from the prelude so normal module code does not couple itself to catalog internals by default.

## Compatibility-only surface pending migration evidence

The crate root and public modules also expose implementation-oriented or dependency-backed symbols that should not be copied into new code without a concrete need:

- public module paths: `bundle`, `error`, `locale`, `macros`, `messages`;
- dependency re-exports: `FluentValue`, `LanguageIdentifier`;
- low-level locale mutation helpers: `push_locale_candidate`, `push_unique`;
- the concrete `FluentCatalog` representation, which exposes `fluent-bundle` storage details.

They remain public today because removing or wrapping them without workspace consumer evidence would create an avoidable pre-1.0 migration surprise.

## Workspace consumer evidence

Use the repository-owned inventory instead of GitHub code-search when reviewing a compatibility-only export:

```text
cargo xtask i18n-api-inventory
cargo xtask i18n-api-inventory --json
```

The command uses `cargo metadata --no-deps` to discover workspace packages that actually declare a dependency on `rustok-ui-i18n`, including renamed dependencies. It then scans those packages' Rust sources for compatibility-only symbol/module candidates and emits deterministic, sorted findings.

Evidence is deliberately conservative:

- `direct-path` means the dependency alias and compatibility symbol occur on the same source line;
- `same-file-candidate` covers multiline use trees and other cases where the source file references the i18n dependency alias and the compatibility symbol, but the textual scanner cannot prove they belong to the same Rust path;
- the command excludes `target`, `node_modules`, `.next`, `output`, symlink traversal, and the `rustok-ui-i18n` package itself;
- a finding is migration evidence to inspect, not an automatic deprecation decision;
- zero findings are useful repository evidence but still do not authorize an automatic breaking removal without review of generated/external consumers.

This inventory exists specifically so pre-1.0 API decisions do not depend on the availability or freshness of an external code-search index.

## Change policy before 1.0

1. Prefer additive facade APIs over removing existing exports.
2. Treat public dependency types, module paths, helper functions and enum variants as compatibility surface.
3. Keep public error enums non-exhaustive so typed diagnostics can evolve without forcing exhaustive downstream matches.
4. Before narrowing an existing export, run `cargo xtask i18n-api-inventory`, inspect every candidate finding, and migrate every in-repository consumer first.
5. If an export still needs removal after migration, deprecate it first when practical and perform the removal only in an explicit pre-1.0 breaking window.
6. Do not move framework, transport, routing, cookie/header or locale-selection policy into this crate merely to make the facade look smaller.

## Review checklist for new public items

Before adding another `pub` item, answer all of the following:

- Is the capability needed by module/application consumers rather than only by this crate?
- Can the behavior be expressed through an existing facade type instead?
- Does the signature expose a third-party type that becomes part of RusTok's compatibility contract?
- Does the item preserve the framework-agnostic, in-memory boundary?
- Is strict/fail-closed versus lenient/fail-soft behavior explicit?
- Is request-controlled input bounded where this crate owns the parsing boundary?

If the answer reveals an implementation detail rather than a consumer contract, prefer `pub(crate)` from the start.
