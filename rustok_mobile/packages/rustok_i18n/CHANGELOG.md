# Changelog

## Unreleased

* Make regional catalog fallback symmetric when CLDR confirms script equivalence: en-Latn-GB can find en-GB before generic parents. Preserve variants in equivalent candidates in both directions; retain exact-key priority and forbid dropping conflicting/unknown regional scripts.
* Shared lookup/negotiation/lazy preparation may now select or load additional equivalent regional catalogs. Canonical-prefix exclusions remain unchanged. Add eight unexecuted core regressions, including regional date rendering.

* Recheck likely-subtags integration: unknown script/region markers preserve exact catalog priority but no longer block normalized fallback, lazy loading or provenance.
* Reject generator output collisions and overwriting either input before any reads/writes. Add five Dart and two Python regressions (not run); pinned artifact reproducibility check passes, not a runtime conformance claim.

* Replace the regional script shortlist with all 7,788 CLDR 48.2 likely-subtag mappings. Add `maximizeLocale` with ordered UTS #35 lookup, unknown-marker handling, explicit-field preservation and nullable no-match behavior.
* Generate all 37 RTL script codes from CLDR metadata; use likely subtags for regional direction and negotiation. Explicit script overrides are preserved.
* Regional fallback chains may now include additional script catalogs; lazy loaders must handle absent candidates. Exact identities, generic language catalogs and configured default fallback remain distinct from inference.
* Add hash-pinned offline generator, original CLDR corpus fixture and ten unexecuted tests (eight core scenarios, one full-corpus test, one widget). Tables were generated; no tests or conformance verifier were run.

* Ninth audit pass: prevent sibling locale matches across conflicting region-implied scripts; reuse the canonical fallback map. Correct sr-ME to Latin using CLDR 48.2. Generic/default fallback and explicit-before-wildcard policy remain unchanged.
* Align typegen required parameters with ignored term/trailing function arguments. Validate class names in both CLI and direct generator API; reject String as well as UiMessages to prevent type shadowing. Regenerated signatures can lose previously spurious required parameters.
* Add nine negotiation/fallback and four CLI/typegen regressions, not executed.

* Eighth audit pass: preserve quoted number-pattern literals, quoted semicolons, doubled apostrophes and literal percent/minus signs. Fraction-pattern extraction ignores quoted text; numeric skeleton syntax is no longer removed from literal affixes.
* Apply default CLDR currency spacing at Unicode symbol boundaries, including non-ASCII labels and directional prefixes; preserve explicit spaces, omit automatic spacing beside non-finite values. `isAlphabeticSymbol` recognizes Unicode letters.
* Add eight unexecuted core regressions; synchronize generator code without executing/regenerating its tables. Document the supported affix-pattern subset.

* Seventh audit pass: use shared locale parsing for number/percent/currency symbols; preserve regional overrides with scripts, variants and extensions. Invalid tags fall back to English. Synchronize the generator template without regenerating CLDR data.
* Replace each escaped surrogate scalar with U+FFFD instead of emitting invalid UTF-16; retain astral scalars and out-of-range parse recovery.
* Add ten unexecuted core regressions, including term/reference scope and catalog fallback boundaries. Unicode numbering-system overrides remain unsupported.

* Resolver semantics audit: typed string/numeric selectors; exact keys use original numbers rather than rendered/rounded text. Numeric strings no longer implicitly select numeric keys; invalid/unknown function selectors use the default variant.
* Preserve and override options across nested NUMBER/DATETIME; evaluate reference arguments and numeric epoch wrappers. Honor currency integer padding and maximum-only fraction settings.
* Correct tiny exponential CLDR operands, exact fraction modulo on web, minimum-native-integer modulo and normalized Portuguese regional lookup. Document raw-number plural categories (not NUMBER display-option-aware).
* Add fourteen core and one wrapper regression; update Python tooling models. Tests/verifiers not run during this audit.

* Date/time audit: canonical locale lookup and en-GB profile; CLDR 48.2 narrow/stand-alone names, locale-specific short-year widths, and Polish day padding. Include the Unicode data notice in LICENSE.
* Fix standalone month case/abbreviations and numeric component padding; ignore invalid date-option strings before style expansion; preserve the sign in two-digit negative proleptic years. Valid explicit components continue to override styles. Document the supported subset rather than claiming ICU parity.

* Add `parseAcceptLanguageStrict`, retaining exclusions/wildcards and reporting typed raw-input limit failures. Keep the existing parser as the accepted-only, empty-on-overflow projection. Both now reject malformed locale tags and return canonical tags.
* Enforce UTF-8 byte limits before trimming and count all raw language ranges. Negotiation honors `q=0`, canonical defaults and script-aware sibling matching, and returns null when no valid supported locale is allowed.
* Canonicalize extension singleton ordering (private use last) and apply the core locale limit in raw UTF-8 bytes. Document negotiation compatibility changes and correct the preferred-locale example.

* Bound parser nesting (junk recovery) and shared per-call resolver depth/steps/character work. Excessive resolution returns `{???}` for the entire message/term without partial text or BiDi delimiters.
* Guard invalid epoch arguments and non-finite numeric options; scale percentages through decimal digits, and avoid minimum-native-integer `abs()` overflow in decimal/currency formatting.
* Snapshot `FtlResource.entries` to keep root entries consistent with cached maps. Synchronize the number-test generator model with exact decimal percent scaling and non-finite sign/affix handling.

* Second audit pass: preparation returns `FutureOr<void>` so eager/fully cached catalogs do not force asynchronous loading. Callers should `await` the result instead of assuming a Future. Lazy preparation validates every locale input before loading.
* Fix infinity signs and non-finite percent layout; bound explicit attribute queries, resolve dotted attributes before lossy aliases, and classify overlapping parent/default chains correctly.
* JSON registrations are atomic and reject cyclic/deep maps and flattened-key collisions.

* Add `MessageCatalog.loadForLocale` preparation hook and recursive composite loading. `CompositeCatalog.catalogs` is now an immutable snapshot; `implements MessageCatalog` adapters must implement the hook.
* Reject malformed locale separators/extensions, duplicate variants, and invalid direct lazy-loader input; retain private-use subtags and intermediate variant fallbacks.
* Remove machine-specific `.dart_tool` caches from version control.

* Parser conforms to Fluent 1.0: correct indentation handling for multiline values, values starting with `.`, no trailing newline, spec-only string escapes, junk recovery per entry, first-wins handling of duplicate IDs.
* Term arguments (`-term(case: "gen")`) and function arguments are supported; term arguments do not leak into the term's message scope.
* Resolver: `NUMBER()` with `minimumFractionDigits` / `maximumFractionDigits`; non-finite numeric selectors (`NaN`, `Infinity`) no longer crash and use the default variant.
* `cldr_plurals` now carries the complete CLDR 48 cardinal rule set (all 224 published language tags — 223 distinct rules, since `kok-Latn` shares the rule of `kok` — generated from `cldr-core@48.2.0/supplemental/plurals.json` and validated against the sample values in that file). Previously only a hand-written subset was covered; the English fallback rule (`one` for exactly `1`) is gone, and every language now uses its CLDR rule. Notable additions: `one: i = 0 or n = 1` (Hindi, Bengali, Persian, …), `one: n = 0..1` and `one: n = 1; two: n = 2` groups, Czech/Slovak `many` for visible fractions, Slavic `v`/`f` clauses, Romanian `101`/`1001` → `few`, Arabic `0.0`/`1.0`/`2.0` → `zero`/`one`/`two`, exact millions → `many` for French, Spanish, Italian and Portuguese, Hebrew `two`, and the `pt-PT` rule that differs from `pt`. Unknown tags return `other`.
* Parser: a term, attribute or select variant without a value is junk; an unescaped `}` is junk in every context; variant keys must be an identifier or a number; attributes may follow blank lines; a pattern's trailing whitespace is trimmed.
* Parser: tabs are content, never layout (`key =\tvalue` keeps the tab, `key\t= value` and tab indentation are junk); comment blocks require one space after the marker, keep the level they started with, and expose it as `FtlComment.level` (`#` → `0`, `##` → `1`, `###` → `2`).
* Parser: entry and comment blocks must start at column 0, and blank lines may separate a value from its attributes.
* Resolver: a message reference inherits the caller's variables (terms still get their own scope).
* Resolver: BiDi isolation matches the reference implementations — a single-placeable pattern and a string/number literal placeable are not isolated.
* Resolver: `NUMBER()` keeps full integer precision above 2^53 and honours `minimumFractionDigits` for integers (`3` → `3.00`); values Dart would print with an exponent (`1e+21`, `1e-7`) are written out in full (and grouped) instead of falling back to exponential notation.
* Accept-Language: ranges keep their original position for equal `q` values, and `q=0`, malformed or duplicated `q` ranges are skipped instead of affecting later ranges (`q` accepts `0`, `1`, `0.<0-3 digits>` and `1.<0-3 zeroes>`).
* Regression tests for tab handling, value trimming, junk rules, callee rules, isolation, scope inheritance, integer precision and CLDR fractions (`test/rustok_i18n_test.dart`).
* Parser: nested placeables (`{ { $x } }`, `{NUMBER({$n})}`, `{-term({$x})}`) are inline expressions and resolve to the value they wrap; a nested placeable as a *selector* is junk, as the reference implementations require.
* Parser: translator comments — a single-`#` comment directly above a message or a term is attached to it (`FtlMessage.comment`, `FtlTerm.comment`) instead of staying a standalone entry; group and resource comments (`##`, `###`) never attach, and a comment followed by a blank line or by junk stays standalone.
* Parser: junk keeps the source verbatim (line ends and following blank lines included) and ends only at a line that starts with a letter, `-` or `#`, per `spec/fluent.ebnf`; `python-fluent`'s Syntax 0.4 rules (a `//` or `[[` line also ends a junk, `[[` does not start a variant) are deliberately not implemented, matching `@fluent/syntax` 0.19 and `fluent-syntax`.
* Parser: a comment marker that ends the file (`#` with no space and no line end) is junk, and `#`/`##`/`###` blocks are grouped by exact level.
* Parser: an out-of-range Unicode escape (`"\U110000"`) yields a junk entry with a diagnostic instead of throwing (`@fluent/syntax` 0.19 raises an unhandled `RangeError` for it).
* Conformance suite: `test/fluent_conformance_test.dart` (254 sources — the `fixtures_reference` set of `@fluent/syntax` 0.19 plus curated edge cases) compares the parser output with the reference ASTs; the same corpus is in `test/data/ftl_reference_corpus.json`. Three intentional deviations (CRLF normalization, escapes decoded while parsing, the out-of-range escape above) are covered by hand-written tests in the same file.
* `LocaleTag`: grandfathered BCP-47 tags are canonicalized to the preferred value of the IANA registry (`i-klingon` -> `tlh`, `en-GB-oed` -> `en-GB-oxendict`, `zh-min-nan` -> `nan`, `sgn-BE-FR` -> `sfb`, `no-bok` -> `nb`, `art-lojban` -> `jbo`, `zh-guoyu` -> `cmn`); the five tags without a preferred value (`zh-min`, `i-default`, `i-enochian`, `i-mingo`, `cel-gaulish`) stay opaque instead of failing to parse.
* CLI `typegen`: message values *and* attributes are generated (attributes call `tAttr` and are named `<key><Attribute>`, e.g. `loginFormPlaceholder`), translator comments become `///` doc comments, and the member/parameter collision checks cover attributes too.
* `cldr_plurals` (audit round 4): two defects in the generated table are fixed — 39 case bodies ended with an `if (...) return ...;` and no final return, which Dart rejects (`The body might complete normally...`) and which made values that matched no clause fall through into the *next* language's rule (Slovenian `5` used the Tzm rule, Russian `1.5` the Polish one); every one of them now ends with `return 'other';`, which is CLDR's catch-all. The 34 `other`-only languages (`ja`, `zh`, `th`, `vi`, `id`, `ko`, …) had no body at all and therefore followed Breton's `one`/`two`/`few`/`many` rules; they now return `other`. The table is guarded by `rustok_mobile/tooling/scripts/verify_cldr_plurals.py`, which reads the Dart switch, fails on a body that can complete normally, and evaluates the rules against the CLDR sample values (9570 samples, 0 mismatches), and by `rustok_mobile/tooling/tests/test_cldr_plurals_table.py`.
* CLI `typegen` (audit round 4): a method's parameters now cover the variables of the messages it references, because a message reference is resolved in the caller's scope (`wrapped = { greeting }` next to `greeting = Hello { $name }` generates `wrapped({required dynamic name})`); reference cycles are followed once, so generation stays finite. Members that would collide with the generated class or with `Object` (`locale`, `toString`, `hashCode`, `runtimeType`, `noSuchMethod`) get a `Message` suffix, and a `locale` variable becomes `localeArg`, so it can no longer shadow the instance locale — the previous output declared a duplicate `locale` member (invalid Dart) or passed the message argument as the bundle locale. `--className UiMessages` is rejected instead of shadowing the catalog type, the summary line no longer claims that attribute-only messages are skipped, and the generated smoke class is compiled by `dart analyze` in CI.
* CLI `check`: when `--recursive` finds several files for the baseline locale, it reports which one is used instead of silently picking the first.
* `LocaleTag`: validates variants, canonicalizes legacy `iw`/`in`/`ji`, and includes extensions in equality. `ha` and `ku` are LTR by default; RTL is decided by script.
* `TextDirection` typedef removed in favour of `LocaleTextDirection` (avoids clashes with Flutter's `TextDirection`).
* Catalogs: locale keys are canonicalized (`ru_RU` == `ru-RU`); a missing attribute never falls back to the main message; argument substitution is single-pass; `null` JSON values are skipped.
* `negotiateLocale` matches a language-only range to a regional locale (`ru` -> `ru-RU`).
* Provenance is computed from canonical locale forms.
* CLI `check` reports counts of missing and extra keys, compares attributes in both directions (missing and extra), and reports parser diagnostics. The key/attribute contract is read from the parsed AST. CLI `typegen` detects member and parameter name collisions, validates the class name, and reports invalid input.
* Removed unused dependencies `petitparser` and `intl`.
* `NUMBER()` formats digits with locale grouping and decimal separators. **Behaviour change:** `1234.5` now renders as `1,234.5` in English (previously `1234.5`). Pass `useGrouping: "false"` to keep the old digits-only output.
* `NUMBER()` supports `style: "percent"`, `minimumIntegerDigits`, and `useGrouping`.
* `NUMBER()` locale data is generated from CLDR 48 (`tooling/scripts/gen_number_symbols.py` -> `lib/src/format/number_symbols.dart`): 416 entries covering every locale whose numbers differ from English, looked up by full tag, then language, then English. **Behaviour change:** grouping and separators now follow CLDR for every locale (Polish, Spanish, Czech and Estonian no longer group four-digit numbers, `fr-CH` groups with an apostrophe, `en-DE`/`ar-EG`/`es-MX` use their own layout), locales with their own numbering system write their own digits (`ar-EG` is `١٬٢٣٤٫٥٠`), and the locale minus sign is used (`−` in Estonian, Basque and Persian, `؜-` in Arabic), as `Intl.NumberFormat` does. The table was verified against `Intl.NumberFormat` for every CLDR locale: 10 724 comparisons, with the only differences coming from runtimes whose ICU carries older CLDR data (`fr-CH`).
* `NUMBER(style: "percent")` uses the locale's CLDR percent pattern (`#,##0%`, `#,##0 %`, `%#,##0`, `% #,#0;% -#,#0` for Welsh) and its percent sign (`٪` in Arabic), and takes the default fraction digits from that pattern, so `0.256` is `26%` in English and `26 %` in German. A `;`-separated negative subpattern is honoured.
* `NUMBER(style: "currency")` places and separates the currency with the locale's CLDR currency pattern instead of a language list: `$1,234.50`, `1 234,50 €`, `1.234,50-` (Faroese), `‏١٬٢٣٤٫٥٠ €`. Alphabetic symbols are separated from the amount by a no-break space as CLDR `currencySpacing` requires (`CHF 3.00`, `USD 1,234.50` for `currencyDisplay: "code"`), and fraction digits come from CLDR `currencyData` (0 for JPY, 3 for KWD, 4 for CLF). **Breaking:** `currencySymbolPrecedes` is gone — the layout is data now — and the symbol table gained the currencies of the CIS, Europe, Asia and Latin America written as CLDR writes them in the currency's own locale.
* `test/number_format_test.dart` (998 cases: 884 numbers and 114 percents) and `test/currency_format_test.dart` (291 cases) assert the formatter against `Intl.NumberFormat` for the same locales and options; both are generated by `rustok_mobile/tooling/scripts/gen_number_tests.py`.
* `CURRENCY()` implemented as `NUMBER` style `currency` with symbols, ISO 4217 fraction digits and locale placement.
* `DATETIME()` implemented with `dateStyle`/`timeStyle` and component options (en, ru, uk, de, fr, es, pl).
* `LazyCatalog` with asynchronous per-locale loading, `loadLocale` / `loadForLocale`, and retry after failure.
* Example locale files in `example/locales` are used by CI (`check` and `typegen`).
* The CLI logic moved to `lib/src/cli/i18n_cli.dart` (`runI18nCli`) so it can be tested. `bin/i18n_cli.dart` is a thin wrapper. The CLI is not exported from the package barrel, so `dart:io` stays out of the library API.
* CLI tests in `test/cli_test.dart`, including a byte-exact golden for the generated class and tests for the reserved member renames, the `localeArg` parameter, the parameters of referenced messages and reference cycles.
* `rustok_mobile/tooling/tests/test_rustok_i18n_package.py` guards the generator's reserved names, its reference walk and the shape of the golden sample; the package workflow gained a `tooling` job for these SDK-free checks.

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
