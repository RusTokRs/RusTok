# rustok_i18n

[![pub package](https://img.shields.io/badge/pub-v0.1.0-blue.svg)](https://pub.dev)
[![Dart SDK](https://img.shields.io/badge/dart-%3E%3D3.0.0-0175C2.svg)](https://dart.dev)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

High-performance, pure Dart internationalization engine implementing the **Mozilla Project Fluent 1.0** specification with multi-catalog support, CLDR pluralization, BiDi isolation, and bounded DoS protection.

---

## Features

* **Project Fluent 1.0 Standard**: Full support for `.ftl` resources, multiline indented patterns, attributes (`.placeholder`), variables (`$count`), terms (`-brand-name`), and select expressions.
* **CLDR Plural Rules**: The full CLDR 48 cardinal rule set (every one of the 224 tags `cldr-core@48.2.0` publishes, 223 distinct rules — `kok-Latn` shares the rule of `kok` — generated from `cldr-core@48.2.0/supplemental/plurals.json` and validated against the sample values shipped with it): *1 товар*, *3 товара*, *5 товаров*, *1.5 → other* in Polish, *2 → two* in Hebrew, *0 → zero* in Welsh, *0.5 → one* in Danish, *i = 0* → `one` in Hindi. Unknown tags resolve to `other`.
* **Exact Numeric Matching**: Exact selectors (`[0]`) strictly take precedence over CLDR plural categories (`[other]`) for clean, natural translations (e.g. *"No unread messages"* vs *"0 unread messages"*).
* **BCP-47 Candidate Fallback Chain**: Ordered locale candidates derived from the requested tag, with likely scripts for regional tags (`ru-RU` -> `ru` -> `en`, `zh-TW` -> `zh-Hant-TW` -> `zh-Hant` -> `zh` -> `en`).
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
  // O(1) direct lookup, 0 new allocations
}
```

### 4. HTTP `Accept-Language` Negotiation (Server / Shelf / Dart Frog)

```dart
final preferred = preferredLocaleFromAcceptLanguage(
  'fr-FR, ru-RU;q=0.9, en-US;q=0.8, *;q=0.5',
);
print(preferred); // 'ru-RU'

final negotiated = negotiateLocale(
  acceptLanguageHeader: request.headers['accept-language'],
  supportedLocales: ['en', 'ru', 'de'],
  defaultLocale: 'en',
);
print(negotiated); // 'ru'
```

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
`--className UiMessages` is rejected because the generated file holds one.

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

* Implemented: Fluent 1.0 messages, attributes, terms with arguments, select expressions, nested placeables, translator comments (a single-`#` comment directly above a message or term is attached to it as `FtlMessage.comment` / `FtlTerm.comment`), and junk recovery (an invalid entry becomes a diagnostic; the rest of the resource still loads with the junk kept verbatim, line ends included).
* Parser strictness follows the grammar in `projectfluent/fluent` (`spec/fluent.ebnf`): entries and comment blocks start at column 0, indentation is made of spaces only (a tab is content, never layout), a term/attribute/variant without a value is junk, `}` must be escaped as `{ "}" }`, variant keys must be an identifier or a number, and trailing whitespace of a pattern is trimmed. Nested placeables (`{ { $x } }`, `{NUMBER({$n})}`) are valid and resolve to the wrapped value; message references as selectors and term attributes outside selectors are rejected, as in the reference implementations.
* Where the reference implementations disagree, the grammar and `@fluent/syntax` win: a junk entry ends only at a line that starts with a letter, `-` or `#` (`python-fluent` also ends it at `//` and `[[` for Syntax 0.4 compatibility), and a junk ends before the line the error was reported on only when that line starts an entry. `python-fluent` also lets `[[` start a variant; here it is a syntax error.
* Conformance is executable: `test/fluent_conformance_test.dart` checks 254 sources — the `fixtures_reference` set of `@fluent/syntax` 0.19 plus curated edge cases — against the AST the reference implementation produces (junk boundaries, patterns, variant keys, comments and expressions included). The same corpus is in `test/data/ftl_reference_corpus.json`.
* Locale data is executable too: `test/number_format_test.dart` (998 cases, including the percent style) and `test/currency_format_test.dart` (291 cases) compare `NUMBER()` with `Intl.NumberFormat` for the same locale and options — grouping, separators, minus signs, digits, percent layout and currency placement. Both files are generated by `rustok_mobile/tooling/scripts/gen_number_tests.py`, which only writes cases where ICU and the CLDR 48 table agree; `rustok_mobile/tooling/scripts/gen_number_symbols.py` regenerates the symbol table itself.
* `NUMBER()`: fraction digits (with full integer precision), locale-aware grouping and decimal separators, `minimumIntegerDigits`, `useGrouping`, locale minus signs (`−` in Estonian, Basque and Persian, `؜-` in Arabic) and localised digits, plus `style: "percent"` with the locale's CLDR percent pattern (`26%`, `26 %`, `%26`). All of it comes from a CLDR 48 table generated into `number_symbols.dart`: 416 entries covering every locale whose numbers differ from English, with a fallback chain of full tag -> language -> English. `rustok_mobile/tooling/scripts/gen_number_symbols.py` regenerates the table. Values that Dart would print with an exponent (`1e+21`, `1e-7`) are written out in full before grouping.
* Plural selection runs on the numeric value a selector resolves to (a `num` variable, a literal or `NUMBER(...)`) after exact string and exact numeric variant keys. A `num` keeps no visible trailing zero, so `1.0` and `1` are the same value: languages whose rule depends on *visible* fraction digits (Slovenian `v != 0 → few`, Czech `v != 0 → many`) select the integer category. Pass the value as a **string** when that distinction matters — a string selector is first matched against variant keys verbatim.
* `DATETIME()`: `dateStyle`, `timeStyle`, and the `weekday`, `year`, `month`, `day`, `hour`, `minute`, `second` and `hour12` options, for en, ru, uk, de, fr, es and pl (English for other languages). Accepts `DateTime`, ISO-8601 strings, and epoch milliseconds (read as UTC). `timeZone: "UTC"` converts to UTC; other zones are not supported, and time zone names are not rendered.
* `LazyCatalog`: asynchronous, on-demand loading of each locale (use it for remote or asset sources). Lookups stay synchronous, so load a locale before using it. Use `loadForLocale` to load the whole fallback chain.
* `CURRENCY()` (as `NUMBER(..., style: "currency", currency: "USD")`): the layout comes from the locale's CLDR currency pattern — `$1,234.50`, `1 234,50 €`, `1.234,50-` (Faroese), `١٬٢٣٤٫٥٠ €` — including `;`-separated negative subpatterns and the CLDR `currencySpacing` no-break space between an alphabetic symbol and the amount (`CHF 3.00`). Fraction digits come from CLDR (`0` for JPY, `3` for KWD, `4` for CLF). `currencyDisplay: "code"` shows the ISO code. Unknown codes show the ISO code.
* Currency symbols are a curated table of common ISO codes written as CLDR writes them in the currency's own locale; the disambiguated forms some locales use for foreign money (`US$` in Armenian, `JP¥` in Basque) are not modelled, and an unknown code falls back to the ISO code. Not implemented further: non-Gregorian calendars, currency names, and the long/short unit and currency-name formats. There is no runtime dependency on `intl` or any other data package — all CLDR data is vendored as Dart tables (`cldr_plurals`, `number_symbols`).
* Grandfathered tags without a preferred value (`zh-min`, `i-default`, `i-enochian`, `i-mingo`, `cel-gaulish`) are kept as an opaque `language`; extended-language tags (`zh-yue`, `zh-cmn-Hans`) are not folded into their preferred form and are rejected as malformed.
* The parser is hand-written; the package has no runtime dependency on PetitParser or `intl`.
* Duplicate message IDs within a locale keep the first definition and report a diagnostic.

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
