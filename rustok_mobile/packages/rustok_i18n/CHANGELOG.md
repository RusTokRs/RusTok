# Changelog

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
