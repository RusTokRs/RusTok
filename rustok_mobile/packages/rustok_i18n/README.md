# rustok_i18n

[![pub package](https://img.shields.io/badge/pub-v0.1.0-blue.svg)](https://pub.dev)
[![Dart SDK](https://img.shields.io/badge/dart-%3E%3D3.0.0-0175C2.svg)](https://dart.dev)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

High-performance, pure Dart internationalization engine implementing the **Mozilla Project Fluent 1.0** specification with multi-catalog support, CLDR pluralization, BiDi isolation, and bounded DoS protection.

---

## Features

* **Project Fluent 1.0 Standard**: Full support for `.ftl` resources, multiline indented patterns, attributes (`.placeholder`), variables (`$count`), terms (`-brand-name`), and select expressions.
* **CLDR Plural Rules**: The full CLDR 48 cardinal rule set (every one of the 224 tags `cldr-core@48.2.0` publishes, 223 distinct rules — `kok-Latn` shares the rule of `kok` — generated from `cldr-core@48.2.0/supplemental/plurals.json` and historically checked against every sample value shipped with it by `rustok_mobile/tooling/scripts/verify_cldr_plurals.py`: 9570 samples, 0 mismatches; not rerun after the current audit): *1 товар*, *3 товара*, *5 товаров*, *1.5 → other* in Polish, *2 → two* in Hebrew, *0 → zero* in Welsh, *0.5 → one* in Danish, *i = 0* → `one` in Hindi. Unknown tags resolve to `other`.
* **Exact Numeric Matching**: Exact selectors (`[0]`) strictly take precedence over CLDR plural categories (`[other]`) for clean, natural translations (e.g. *"No unread messages"* vs *"0 unread messages"*).
* **BCP-47 Candidate Fallback Chain**: Ordered locale candidates derived from the requested tag, with CLDR likely scripts for regional tags (`ru-RU` -> `ru-Cyrl-RU` -> `ru-Cyrl` -> `ru` -> `en`, `zh-TW` -> `zh-Hant-TW` -> `zh-Hant` -> `zh` -> `en`).
* **Legacy Android / Java Code Canonicalization**: Automatic mapping of deprecated ISO 639-1 language tags (`iw` -> `he`, `in` -> `id`, `ji` -> `yi`) and of the grandfathered BCP-47 tags of RFC 5646 (`i-klingon` -> `tlh`, `en-GB-oed` -> `en-GB-oxendict`, `zh-min-nan` -> `nan`).
* **Cycle & Recursion Guard**: Prevents `StackOverflowError` crashes when messages or terms cross-reference each other circularly, gracefully returning fallback tokens (`{???}`).
* **Unicode BiDi & RTL Support**: Built-in script and language text direction detection (`LocaleTextDirection.ltr` / `LocaleTextDirection.rtl`) and automatic directional isolation (`\u2068...\u2069`) to protect layout integrity.
* **Bounded Security & DoS Protection**: Strictly bounded inputs (locale tags <= 64 bytes, message keys <= 256 bytes (`MAX_MESSAGE_KEY_LEN` parity with `rustok-ui-i18n`), Accept-Language headers <= 4096 bytes).
* **High-Performance Batch Lookups**: `UiLocaleTranslator` pre-computes candidate hierarchies once, reusing the candidate list during high-frequency list rendering (map lookups per candidate).
* **Multi-Catalog Architecture**: Mix and match `FluentCatalog` (.ftl), `StaticCatalog` (Map tables), `JsonCatalog` (nested JSON dictionaries), and `CompositeCatalog`.
* **Built-in CLI Tooling**: Includes `i18n_cli` for continuous translation parity checks and compile-time type-safe Dart code generation (`typegen`).

---

## Getting Started

Add `rustok_i18n` to your `pubspec.yaml`:

```yaml
dependencies:
  rustok_i18n: ^0.1.0
```

---

## Usage

### 1. Basic Project Fluent Translation

```dart
import 'package:rustok_i18n/rustok_i18n.dart';

void main() {
  final catalog = FluentCatalog(useIsolating: false);

  catalog.registerFtl('en', '''
-brand-name = RusToK
welcome = Welcome to { -brand-name }!
email-input = Email address
    .placeholder = name@example.com
items-count = { \$count ->
    [0] No items in cart
    [one] 1 item in cart
   *[other] { \$count } items in cart
}
''');

  final messages = UiMessages(
    catalog: catalog,
    defaultLocale: 'en',
  );

  // Simple key lookup
  print(messages.t('welcome')); 
  // Output: Welcome to RusToK!

  // Attribute lookup
  print(messages.tAttr('email-input', 'placeholder')); 
  // Output: name@example.com

  // Dotted attribute syntax shorthand
  print(messages.t('email-input.placeholder')); 
  // Output: name@example.com

  // Plurals and variables
  print(messages.t('items-count', args: {'count': 0})); 
  // Output: No items in cart
  print(messages.t('items-count', args: {'count': 1})); 
  // Output: 1 item in cart
  print(messages.t('items-count', args: {'count': 5})); 
  // Output: 5 items in cart
}
```

### 2. Complex Russian / Slavic Pluralization

```dart
catalog.registerFtl('ru', '''
comments-count = { \$count ->
    [one] { \$count } комментарий
    [few] { \$count } комментария
   *[other] { \$count } комментариев
}
''');

print(messages.t('comments-count', locale: 'ru', args: {'count': 1}));
// Output: 1 комментарий
print(messages.t('comments-count', locale: 'ru', args: {'count': 3}));
// Output: 3 комментария
print(messages.t('comments-count', locale: 'ru', args: {'count': 5}));
// Output: 5 комментариев
print(messages.t('comments-count', locale: 'ru', args: {'count': 21}));
// Output: 21 комментарий
```

### 3. High-Performance Pre-bound Lookups (`UiLocaleTranslator`)

In performance-critical render loops (e.g. ListView item builders or data tables), use `forLocale()` to eliminate redundant candidate chain re-computations:

```dart
final translator = messages.forLocale('ru');

for (final item in items) {
  final label = translator.t('status.${item.status}');
  // Reuses the locale candidate chain; normalization/formatting still allocate.
}
```

### 4. HTTP `Accept-Language` Negotiation (Server / Shelf / Dart Frog)

```dart
final preferred = preferredLocaleFromAcceptLanguage(
  'fr-FR, ru-RU;q=0.9, en-US;q=0.8, *;q=0.5',
);
print(preferred); // 'fr-FR' — extraction does not apply a supported-locale list

final negotiated = negotiateLocale(
  acceptLanguageHeader: 'fr-FR, ru-RU;q=0.9, en-US;q=0.8',
  supportedLocales: ['en', 'ru', 'de'],
  defaultLocale: 'en',
);
print(negotiated); // 'ru'
```

`parseAcceptLanguageStrict(header)` is the lossless preference parser: it keeps
`q=0` exclusions and wildcards, canonicalizes locale tags, and returns an immutable
list sorted by quality and original raw-range position. It throws
`AcceptLanguageLimitException` with `limit == AcceptLanguageLimit.headerBytes` or
`AcceptLanguageLimit.ranges` for whole-header overflow. The bounds are **4096 UTF-8
bytes before trimming** and **64 raw comma-separated ranges**, including empty,
malformed and rejected ranges. Malformed individual preferences are skipped;
unknown parameters remain ignored, matching the Rust owner's tolerance policy.
Underscores are accepted; POSIX `.UTF-8`/`@modifier` suffixes are not HTTP tags.

```dart
final preferences = parseAcceptLanguageStrict('en;q=0, ru;q=0.8');
print(preferences.last.isRejected); // true — the host can inspect exclusions
```

For compatibility, `parseAcceptLanguage` still omits `q=0` and returns an empty
list on whole-header overflow. Accepted-locale projection/preferred helpers keep
this behavior. Their results are canonical, not raw input spellings. Full locale
serialization orders extension singleton blocks consistently, with private use
last, so equivalent extension-block orderings do not create duplicate locales.
The core's 64-byte locale bound also measures raw UTF-8 before normalization.

`negotiateLocale` now returns **null** if its supported list has no valid, allowed
locale; it never invents an unsupported default. It preserves the chosen entry's
original spelling, but compares defaults canonically (`en_US` == `en-US`).
Exclusions also apply to fallback: the most-specific matching canonical catalog
subtag prefix wins, and duplicate ranges keep their highest quality. Thus
`en;q=0, en-US;q=0.8` allows `en-US`, while `en-US;q=0, en;q=0.8` does not.
A wildcard exclusion can be overridden by a specific accepted prefix. Candidate
quality is capped by both the requesting preference and the candidate's
most-specific matching range; `en;q=1, en-US;q=0.1, fr;q=0.9` therefore selects
`fr` over `en-US` when those are the available locales. Ties preserve preference
traversal and exact/parent/sibling lookup order. Matching
uses extension-free catalog identities, not formatting preferences. An explicit
script does not match a sibling with a conflicting explicit or known region-implied script. Regional inference uses the shared fallback table; explicit scripts override it. Generic catalogs remain eligible.

This negotiation helper is **host policy**, not a full HTTP content-negotiation
engine or automatic Flutter locale selection. It keeps the existing policy that
explicit preferences precede wildcard/default fallback. Invalid/oversized headers
are treated as absent; hosts that must reject such requests should call the
strict parser first. Supported-list filtering, authentication and tenant policy
remain the host's responsibility.

### 5. Multi-Catalog Setup (Static & JSON)

```dart
final staticCatalog = StaticCatalog({
  'en': {'app.title': 'SuperApp', 'btn.save': 'Save'},
  'ru': {'app.title': 'СуперАпп', 'btn.save': 'Сохранить'},
});

final composite = CompositeCatalog([
  fluentCatalog,
  staticCatalog,
]);

final uiMessages = UiMessages(catalog: composite);
```

---

## CLI Tooling

`rustok_i18n` ships with an integrated CLI for CI/CD and developer workflows:

### Check Locale Parity in CI
Verify that all `.ftl` files in a folder have 100% matching keys against a baseline:

```bash
dart run rustok_i18n:i18n_cli check --dir assets/locales --baseline en --recursive
```

### Type-Safe Code Generation (`typegen`)
Generate compile-time Dart accessor classes from an `.ftl` file:

```bash
dart run rustok_i18n:i18n_cli typegen \
  --input assets/locales/en.ftl \
  --output lib/generated/messages.g.dart \
  --className AppMessages
```

Generated code usage:

```dart
final t = AppMessages(messages, locale: 'en');
print(t.welcome);
print(t.itemsCount(count: 3));
print(t.loginInputPlaceholder); // attribute accessor, calls `tAttr`
```

The generator follows the resolver's scoping rules: a method takes one parameter per
variable the pattern reads, including the variables of every *message* it references
(a message reference is resolved in the caller's scope, so
`wrapped = { greeting }` next to `greeting = Hello { $name }` needs `name` too), while
terms keep their own scope. Names are made safe for Dart: members that clash with the
class API or with `Object` are renamed with a `Message` suffix (`locale` ->
`localeMessage`, `to-string` -> `toStringMessage`), a variable called `locale` becomes
the `localeArg` parameter so it cannot shadow the instance locale, and
`--className UiMessages` and `--className String` are rejected because generated
fields/return types use those types. The direct `generateTypedMessages` API applies
the same validation. Ignored positional term arguments and function arguments
after the first do not create required parameters; named options are literals.

---

## Architecture

| Component | Responsibility |
| :--- | :--- |
| `LocaleTag` | BCP-47 / POSIX parser, normalizer, and directionality detector. |
| `FtlParser` | Recursive-descent AST parser for Project Fluent 1.0 syntax. |
| `FluentBundle` | In-memory message and term registry with BiDi isolation. |
| `cldr_plurals` | CLDR 48 cardinal plural rules (all published language tags). |
| `number_symbols` / `datetime_format` | Locale symbols for `NUMBER()` and `DATETIME()`. |
| `LazyCatalog` | On-demand asynchronous loading of locale sources. |
| `UiMessages` | High-level facade with fallback resolution and provenance tracking. |
| `UiLocaleTranslator` | Pre-computed locale candidate translator for high-speed batch lookups. |
| `AcceptLanguageParser` | RFC 9110 bounded `Accept-Language` quality ranking and negotiation. |

---

## Status and limitations

### Resource and formatting safety

* Parsing is bounded by `maxFtlNestingDepth = 100` combined active placeable/inline-expression frames. Excessive nesting becomes `FtlJunk` with a diagnostic; parsing resumes at the next resource entry. This is an intentional safety restriction on otherwise syntactically valid, extremely deep Fluent sources.
* Every `FluentBundle.format` / `formatTerm` call gets a fresh, shared resolution budget: depth 100, `maxFluentResolutionSteps = 10000` pattern/expression/variant visits (including pattern elements and term arguments), and `maxFluentResolutionCharacters = 100000` cumulative UTF-16 units processed in pattern buffers, selectors, variant keys and numeric/date-string conversion. Message references, terms and selects share the same budget; even acyclic reference expansion or empty expansions cannot reset it. Intermediate copies count, so the final output can be shorter than the character limit when the budget is exhausted.
* Exceeding a resolution budget returns the existing `{???}` error marker for the **whole** formatting operation, never partial text or unbalanced BiDi isolates. It is a found-but-unformattable message, not a missing key: normal locale fallback is not triggered. Load diagnostics still describe parsing/registration, not runtime formatting. Arbitrary exceptions from caller-supplied objects or custom catalog adapters are not swallowed.
* These limits do **not** cap source download size, total resource entries, loader duration or the cost of user-defined `toString()` callbacks. Hosts must bound untrusted asset/network input and loader timeouts, and should pass primitive formatting arguments. Programmatic AST child lists must not be mutated while used; `FtlResource` snapshots its root `entries` list so its cached lookup maps cannot drift.
* Invalid numeric epoch arguments (`NaN`, infinity, or values outside ±8,640,000,000,000,000 milliseconds) use the existing non-applicable `DATETIME()` fallback instead of throwing. Non-finite numeric digit options use the default. Percent scaling shifts decimal digits rather than multiplying a native number, avoiding integer overflow and binary rounding noise; formatting the minimum native signed integer avoids overflowing `abs()`.

### Other capabilities and limitations

* Implemented: Fluent 1.0 messages, attributes, terms with arguments, select expressions, nested placeables, translator comments (a single-`#` comment directly above a message or term is attached to it as `FtlMessage.comment` / `FtlTerm.comment`), and junk recovery (an invalid entry becomes a diagnostic; the rest of the resource still loads with the junk kept verbatim, line ends included).
* Parser strictness follows the grammar in `projectfluent/fluent` (`spec/fluent.ebnf`): entries and comment blocks start at column 0, indentation is made of spaces only (a tab is content, never layout), a term/attribute/variant without a value is junk, `}` must be escaped as `{ "}" }`, variant keys must be an identifier or a number, and trailing whitespace of a pattern is trimmed. Nested placeables (`{ { $x } }`, `{NUMBER({$n})}`) are valid and resolve to the wrapped value; message references as selectors and term attributes outside selectors are rejected, as in the reference implementations.
* Where the reference implementations disagree, the grammar and `@fluent/syntax` win: a junk entry ends only at a line that starts with a letter, `-` or `#` (`python-fluent` also ends it at `//` and `[[` for Syntax 0.4 compatibility), and a junk ends before the line the error was reported on only when that line starts an entry. `python-fluent` also lets `[[` start a variant; here it is a syntax error.
* Conformance is executable: `test/fluent_conformance_test.dart` checks 254 sources — the `fixtures_reference` set of `@fluent/syntax` 0.19 plus curated edge cases — against the AST the reference implementation produces (junk boundaries, patterns, variant keys, comments and expressions included). The same corpus is in `test/data/ftl_reference_corpus.json`.
* Locale data is executable too: `test/number_format_test.dart` (998 cases, including the percent style) and `test/currency_format_test.dart` (291 cases) compare `NUMBER()` with `Intl.NumberFormat` for the same locale and options — grouping, separators, minus signs, digits, percent layout and currency placement. Both files are generated by `rustok_mobile/tooling/scripts/gen_number_tests.py`, which only writes cases where ICU and the CLDR 48 table agree; `rustok_mobile/tooling/scripts/gen_number_symbols.py` regenerates the symbol table itself.
* `NUMBER()`: fraction digits (with full integer precision), locale-aware grouping and decimal separators, `minimumIntegerDigits`, `useGrouping`, locale minus signs (`−` in Estonian, Basque and Persian, `؜-` in Arabic) and localised digits, plus `style: "percent"` with the locale's CLDR percent pattern (`26%`, `26 %`, `%26`). All of it comes from a CLDR 48 table generated into `number_symbols.dart`: 416 entries covering every locale whose numbers differ from English, with canonical locale lookup: extension-free full tag -> reduced variants -> language-script -> language-region -> language -> English. `rustok_mobile/tooling/scripts/gen_number_symbols.py` regenerates the table. Values that Dart would print with an exponent (`1e+21`, `1e-7`) are written out in full before grouping.
* Selectors preserve types: strings match identifier keys verbatim (including whitespace/BiDi); numeric keys match numbers only. `"1"` no longer matches `[1]`: pass a `num` or explicitly use `NUMBER(...)` for conversion. Numeric exact matches use the original value, not rounded/localized/percent display, and retain this package's priority over categories regardless of source order. Missing/invalid selectors use the default variant; unknown-function display fallback is not a selector value.
* Plural categories use the **raw number**, including decimal operands expanded from exponential notation. `NUMBER` rounding/fraction options do **not** change category; trailing-zero scale (`1.0` versus `1`) is not retained. This differs from option-aware Fluent runtimes. If visible scale matters, pass an explicitly computed category identifier; passing a numeric string does not recover its CLDR operands.
* Nested `NUMBER` and `DATETIME` preserve same-function options, with outer options overriding inner ones. Built-in arguments evaluate message/term references; this package's explicit numeric-string conversion applies to their text too. `DATETIME(NUMBER(epoch))` uses the raw epoch, not its display. Currency formatting honors `minimumIntegerDigits` and a maximum-only fraction setting (USD `maximumFractionDigits: 0` removes the default two decimal places).
* `DATETIME()`: `dateStyle`, `timeStyle`, and the `weekday`, `year`, `month`, `day`, `hour`, `minute`, `second` and `hour12` options, for en, en-GB, ru, uk, de, fr, es and pl. Canonical full/region/language profile lookup shares `LocaleTag` normalization; unsupported profiles use language data or English. British English has day-first numeric dates and a 24-hour default. Other regional English profiles still use the base English data; this is not a complete regional CLDR implementation.
* Date/month text widths include `long`, `short` and `narrow` where applicable. CLDR 48.2 Gregorian narrow/stand-alone names and short-date year widths are embedded in `datetime_format.dart` (source: `cldr-dates-full@48.2.0/main/<locale>/ca-gregorian.json`). Month-only output uses stand-alone forms; a day selects date-context forms. Short years are full-width for en-GB, ru, fr and pl, and two-digit for en, uk, de and es. Explicit valid components override style defaults; invalid option strings are ignored **before** expansion, so they cannot silently choose the wrong width or suppress a valid style.
* A standalone numeric day/month/minute/second is unpadded unless `2-digit` is requested. Combined numeric dates use the locale profile's padding; clock groups pad minutes/seconds. Years preserve Dart's signed proleptic Gregorian numbering, including zero and negative years; `2-digit` retains a negative sign rather than wrapping it modulo 100. Era labels are not implemented.
* Date inputs: `DateTime`, ISO-8601 strings, or epoch milliseconds (read as UTC). The formatter preserves the supplied `DateTime`'s local/UTC fields; the Fluent resolver converts to UTC only for `timeZone: "UTC"`. Other zones, time-zone names, non-Gregorian calendars and Unicode formatting-extension preferences are not implemented. Styles are expanded into this package's components, not an ICU skeleton matcher: mixed styles/components, unusual partial-field combinations, date/time join text, period spacing and punctuation may differ from `Intl.DateTimeFormat`. No complete date-format conformance is claimed.
* `MessageCatalog.loadForLocale` returns `FutureOr<void>` (use `await`, not an unconditional `.then`). It prepares catalogs for synchronous lookups: eager catalogs do nothing, `LazyCatalog` loads the whole fallback chain, and `CompositeCatalog` recursively prepares every child and propagates failures after all settle. Eager catalogs and fully cached lazy/composite catalogs complete synchronously so UI hosts need not enter a loading state. Composite priority lists are immutable snapshots; replace the composite to change children. Custom classes using `implements MessageCatalog` must implement this hook (classes extending it inherit the no-op).
* `LazyCatalog` caches successful and absent (`null`) sources, shares concurrent requests, and retries thrown failures. `loadLocale` rejects invalid tags before calling the loader. There is no eviction/refresh API: replace a catalog to refresh cached sources.
* Locale parsing rejects empty subtags, malformed/duplicate extensions and duplicate variants. Private-use suffixes preserve one-character subtags. Variant fallback removes one variant at a time (`sl-rozaj-biske` → `sl-rozaj` → `sl`).
* `CURRENCY()` (as `NUMBER(..., style: "currency", currency: "USD")`): the layout comes from the locale's CLDR currency pattern — `$1,234.50`, `1 234,50 €`, `1.234,50-` (Faroese), `١٬٢٣٤٫٥٠ €` — including `;`-separated negative subpatterns and the default CLDR `currencySpacing` no-break space at a non-symbol currency boundary next to a finite amount (`CHF 3.00`, `Kč 3.00`). Fraction digits come from CLDR (`0` for JPY, `3` for KWD, `4` for CLF). `currencyDisplay: "code"` shows the ISO code. Unknown codes show the ISO code.
* Currency symbols are a curated table of common ISO codes written as CLDR writes them in the currency's own locale; the disambiguated forms some locales use for foreign money (`US$` in Armenian, `JP¥` in Basque) are not modelled, and an unknown code falls back to the ISO code. Not implemented further: non-Gregorian calendars, currency names, and the long/short unit and currency-name formats. There is no runtime dependency on `intl` or any other data package — all CLDR data is vendored as Dart tables (`cldr_plurals`, `number_symbols`).
* Grandfathered tags without a preferred value (`zh-min`, `i-default`, `i-enochian`, `i-mingo`, `cel-gaulish`) are kept as an opaque `language`; private-use-only tags (`x-company`) and extended-language tags (`zh-yue`, `zh-cmn-Hans`) are not folded into their preferred form and are rejected as malformed.
* The parser is hand-written; the package has no runtime dependency on PetitParser or `intl`.
* Duplicate message IDs within a locale keep the first definition and report a diagnostic.
* `JsonCatalog.registerJson` merges atomically. Cyclic maps, more than 100 levels of map nesting, and collisions between flattened keys (for example `a.b` and `a: {b: ...}` in the same registration) throw `FormatException` without changing existing data or supported locales. Separate successful registrations can still overwrite existing keys.
* UI lookup limits include explicit attributes and apply before allocating normalized aliases. Literal keys win, followed by dotted attributes, then lossy key aliases. A requested parent locale remains `parentLanguage` provenance even when it is also configured as the default.
* Non-finite `NUMBER()` values retain infinity's sign and percent affixes; currency formatting uses the same corrected non-finite number path.

---

## License

Library code is licensed under MIT. CLDR-derived locale data carries the Unicode-3.0 notice included in [LICENSE](LICENSE).

### Resolver semantics audit — 2026-10-13

The sixth pass adds fourteen core regressions and one Flutter integration regression for typed selection, nested/reference arguments, currency options and CLDR operand/locale boundaries. These tests and the synchronized Python verification/generation models were **not executed**. Syntax-only checks are not compilation, analyzer or runtime evidence. Full Fluent conformance is not claimed; see the raw-number plural policy above.

### Number-locale and Unicode audit — 2026-10-13

Number/percent/currency symbols now use the shared `LocaleTag` parser, including aliases, POSIX spellings and whitespace normalization. Extensions and private use are stripped for lookup; variants are reduced before script/region/language fallbacks. For example, `es-MX-u-nu-latn` and `es-Latn-MX` retain Mexican decimal/group separators rather than falling back to Spain's defaults. Invalid tags use English rather than a partially parsed language. Unicode `u-nu` numbering-system overrides remain unsupported: the locale's default digits are used.

FTL Unicode escapes in the surrogate range U+D800–U+DFFF now resolve individually to U+FFFD, including adjacent surrogate escapes. A supplementary scalar should be written as a six-digit escape such as `\U01F600` (or literal Unicode). Values above U+10FFFF remain parse errors with recovery at the next entry.

Ten core regression cases were added, not run: regional number profiles and invalid tags, Unicode boundaries/recovery, nested term scope restoration, message references in term scope, attribute fallback and found-but-empty/broken-reference values. The latter remain found values, not permission to fall through to another locale. The number-symbol generator template is synchronized; its CLDR table data was not regenerated. Syntax-only checks do not establish runtime conformance.

### Number-pattern and currency-spacing audit — 2026-10-13

Number affixes now preserve apostrophe-quoted literals (digits, punctuation, `;`, `%`, `-`) and decode doubled apostrophes. Only unquoted semicolons separate positive/negative subpatterns; only unquoted percent/minus signs are localized. `patternFractionDigits` likewise ignores quoted prefix/suffix text. The public `formatFluentNumber(pattern: ...)` argument controls affixes, **not** precision/grouping or scaling: use digit options for precision and the Fluent percent style for scaling. Scientific/significant-digit pattern syntax is not implemented.

Currency spacing uses the default CLDR boundary rule (`[:^S:]` next to a digit), not an ASCII whole-label check. This covers `Kč`, `zł`, punctuation-ended labels and directional prefixes without duplicating existing spaces. Automatic spacing is not added next to NaN/infinity; explicit pattern spaces remain. `isAlphabeticSymbol` now recognizes Unicode letters but does not determine currency spacing.

Eight core regressions added, not run. Both Python generator models/templates are synchronized; generators and conformance suites were not executed, and CLDR data/test tables were not regenerated. Runtime behavior on supported Dart VM/web SDKs still requires maintainer verification.

### Negotiation and typegen contract audit — 2026-10-13

Same-language sibling negotiation compares explicit or known region-implied scripts on both sides: `zh-Hant` no longer picks `zh-CN` as a matching sibling; `sr-Latn` does not pick `sr-RS`. The regional fallback map was reused rather than copied, and its `sr-ME` entry corrected to Latin using CLDR 48.2 `likelySubtags.json`. This historical implementation is superseded by the full table below. Generic language catalogs remain usable. The full-table change below replaces the limited map; q=0 matching remains a canonical-prefix rule, and configured default/first-supported fallback may still choose another script when no preference matched. The existing explicit-preference-before-wildcard policy is unchanged.

Typegen now rejects invalid/shadowing class names from both CLI and direct API calls. Required parameters no longer include ignored positional term arguments or ignored trailing function arguments. Callers may need to remove these obsolete named arguments after regenerating their accessors.

Nine negotiation/fallback regressions and four CLI/typegen regressions were added, not run. CLDR input was inspected as data, not executed. SDK compilation, analyzer, generation and runtime/conformance tests remain unverified.

### Complete likely-subtags data and maximization

`maximizeLocale(LocaleTag)` implements **Add Likely Subtags** from UTS #35, release 48, with all **7,788** mappings from `cldr-core@48.2.0`. Lookup order is language-script-region, language-script, language-region, language. Missing fields and the unknown markers `und`, `Zzzz`, `ZZ` are filled; explicit known fields are preserved. Variants and extensions survive as immutable snapshots. Unknown incomplete languages return `null` rather than being guessed as English; complete tags are retained.

```dart
final input = LocaleTag.parse('az-IR');
final maximal = maximizeLocale(input); // az-Arab-IR
print(input.toCatalogTag());           // az-IR (unchanged)
print(input.isRtl);                    // true
```

The function expects canonical input under the existing `LocaleTag` contract. It does not add full CLDR alias rewriting, locale minimization, language-distance matching or parentLocales inheritance. Maximization is a probabilistic inference, not evidence of a person's language or script. Supported language-only catalogs remain generic in negotiation, as do scriptless language-only requests. Regional script compatibility uses the complete likely-subtags table; explicitly configured fallback remains a separate policy.

**Integration and compatibility:** the former hand-maintained regional table is removed. Every known scriptless regional locale can gain script-specific fallback candidates, e.g. `en-GB → en-Latn-GB → en-Latn → en`. Exact catalog identities remain first and registrations are not renamed. Language-only catalog chains do not acquire an inferred region. `LazyCatalog` preparation can therefore request additional locale resources; loaders should return `null` for absent resources. `LocaleTag.direction` and the standalone direction helper use inferred scripts; all **37** RTL scripts are generated from CLDR script metadata. An explicit script wins; unrecognized scripts/languages default to LTR.

Data is bundled in Dart with no network access or additional runtime dependency. Generated data and the source test fixture are covered by the Unicode-3.0 notice in LICENSE. Regenerate from the pinned npm archive:

```sh
npm pack cldr-core@48.2.0
mkdir -p /tmp/cldr-core-48.2
tar -xzf cldr-core-48.2.0.tgz -C /tmp/cldr-core-48.2
python3 rustok_mobile/tooling/scripts/gen_likely_subtags.py \
  --input /tmp/cldr-core-48.2/package/supplemental/likelySubtags.json \
  --scripts /tmp/cldr-core-48.2/package/scriptMetadata.json
# Add --check to compare both generated files without writing.
```

The generator checks the exact SHA-256 of both sources and sorts output deterministically. Its default paths are independent of the working directory. During this change the tables were generated from these pinned inputs; tests and the `--check` verification were **not** run. Added eight core regression cases, one corpus test over every mapping (including idempotence), and one Flutter direction regression. The corpus fixture is the original pinned JSON, not an output calculated by the Dart implementation. Syntax checks are not compilation or conformance evidence; maintainers must run Dart/Flutter suites, including web coverage, before release.

Specification: https://github.com/unicode-org/cldr/blob/release-48/docs/ldml/tr35.md#likely-subtags

#### Likely-subtags implementation recheck

The UTS #35 release-48 lookup order was re-reviewed, including a specific L-S-R
match and script-versus-region precedence. The corpus test remains unexecuted.
Two integration/tooling issues were corrected:

* `Zzzz` and `ZZ` no longer mask fallback inference: `pa-Zzzz-PK` tries its exact
  key first, then `pa-PK`, `pa-Arab-PK`, `pa-Arab`, `pa`. Unknown regions do not
  introduce guessed real-region catalogs (`en-ZZ` falls back to `en`). This same
  chain drives provenance and lazy preparation.
* The generator rejects overlapping output paths or an output that would
  overwrite either pinned input, before reading or writing data.

Five Dart regressions and two Python generator-guard regressions were added,
not run. The generator's **file-reproducibility `--check`** was run against the
committed original likely-subtags fixture and the hash-pinned CLDR script metadata;
both generated Dart files matched. This compares artifacts only; no Dart/Flutter
or Python unit tests, compilation, analyzer or runtime conformance were executed.

#### Symmetric regional catalog fallback

Regional script equivalence now works in both directions. For example,
`en-Latn-GB → en-GB → en-Latn → en`, so adding an explicit default script no
longer causes British translations/date formats to be skipped in favor of
plain English. Exact catalog spelling remains first. At each variant level,
the CLDR-equivalent regional spelling is tried before discarding that variant:
`de-Latn-DE-1996 → de-DE-1996 → de-Latn-DE → de-DE → de-Latn → de`.
The reverse scriptless chain also retains variants in inferred candidates.

Script omission is allowed only when CLDR's language-region inference agrees
with the explicit script. `az-Latn-IR` therefore does not try `az-IR` (Arab),
and an unknown regional language cannot justify dropping an explicit script.
This is catalog fallback policy, not full CLDR parentLocales inheritance.

The shared chain drives direct/bound translation, provenance, negotiation and
lazy preparation. Additional equivalent resources may be loaded; exact
registrations are not merged or renamed. Negotiation's documented canonical
prefix q=0/quality policy is unchanged (it is not an inferred-equivalence
exclusion policy). Eight new core regressions cover candidate order, variants,
conflicts, lookup/date output, exclusions/weights, lazy preparation and immutable
candidate snapshots. Tests were not run.
