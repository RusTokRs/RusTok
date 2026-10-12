# Changelog

## Unreleased

* Parser conforms to Fluent 1.0: correct indentation handling for multiline values, values starting with `.`, no trailing newline, spec-only string escapes, junk recovery per entry, first-wins handling of duplicate IDs.
* Term arguments (`-term(case: "gen")`) and function arguments are supported; term arguments do not leak into the term's message scope.
* Resolver: `NUMBER()` with `minimumFractionDigits` / `maximumFractionDigits`; non-finite numeric selectors (`NaN`, `Infinity`) no longer crash and use the default variant.
* CLDR plural rules extended and completed for Czech, Slovak, Croatian, Serbian, Bosnian, Slovenian, Lithuanian, Latvian, Romanian, French, and Portuguese.
* `LocaleTag`: validates variants, canonicalizes legacy `iw`/`in`/`ji`, and includes extensions in equality. `ha` and `ku` are LTR by default; RTL is decided by script.
* `TextDirection` typedef removed in favour of `LocaleTextDirection` (avoids clashes with Flutter's `TextDirection`).
* Catalogs: locale keys are canonicalized (`ru_RU` == `ru-RU`); a missing attribute never falls back to the main message; argument substitution is single-pass; `null` JSON values are skipped.
* `negotiateLocale` matches a language-only range to a regional locale (`ru` -> `ru-RU`).
* Provenance is computed from canonical locale forms.
* CLI `check` reports counts of missing and extra keys, compares attributes, and reports parser diagnostics. CLI `typegen` detects member and parameter name collisions, validates the class name, and reports invalid input.
* Removed unused dependencies `petitparser` and `intl`.

## 0.1.0

* Initial release of `rustok_i18n`.
* Mozilla Project Fluent 1.0 AST parser and runtime evaluation engine.
* Multi-catalog architecture: `FluentCatalog`, `StaticCatalog`, `JsonCatalog`, and `CompositeCatalog`.
* 7-tier BCP-47 / Unicode locale candidate fallback chain with inferred scripts.
* CLDR plural rules for Slavic languages (Russian, Ukrainian, Belarusian), Arabic, French, Polish, Portuguese, and CJK languages.
* BiDi text direction detection (`LocaleTextDirection`) and Unicode directional isolation (`\u2068...\u2069`).
* Circular cross-reference cycle guard preventing stack overflow crashes.
* Bounded DoS protection for locale tags, keys, and `Accept-Language` headers.
* High-performance pre-computed candidate translator `UiLocaleTranslator`.
* Integrated CLI (`i18n_cli`) with locale parity validator (`check`) and code generation (`typegen`).
