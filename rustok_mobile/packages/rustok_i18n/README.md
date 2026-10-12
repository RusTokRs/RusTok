# rustok_i18n

[![pub package](https://img.shields.io/badge/pub-v0.1.0-blue.svg)](https://pub.dev)
[![Dart SDK](https://img.shields.io/badge/dart-%3E%3D3.0.0-0175C2.svg)](https://dart.dev)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

High-performance, pure Dart internationalization engine implementing the **Mozilla Project Fluent 1.0** specification with multi-catalog support, CLDR pluralization, BiDi isolation, and bounded DoS protection.

---

## Features

* **Project Fluent 1.0 Standard**: Full support for `.ftl` resources, multiline indented patterns, attributes (`.placeholder`), variables (`$count`), terms (`-brand-name`), and select expressions.
* **CLDR Plural Rules**: Accurate plural category resolution for Slavic languages (Russian, Ukrainian, Belarusian: *1 товар*, *3 товара*, *5 товаров*), Arabic, French, Polish, Brazilian Portuguese, and East Asian languages.
* **Exact Numeric Matching**: Exact selectors (`[0]`) strictly take precedence over CLDR plural categories (`[other]`) for clean, natural translations (e.g. *"No unread messages"* vs *"0 unread messages"*).
* **7-Tier Candidate Fallback Chain**: Robust locale candidate derivation adhering to BCP-47 (`ru-RU` -> `ru` -> `en`, `zh-TW` -> `zh-Hant-TW` -> `zh-Hant` -> `zh` -> `en`).
* **Legacy Android / Java Code Canonicalization**: Automatic mapping of deprecated ISO 639-1 language tags (`iw` -> `he`, `in` -> `id`, `ji` -> `yi`).
* **Cycle & Recursion Guard**: Prevents `StackOverflowError` crashes when messages or terms cross-reference each other circularly, gracefully returning fallback tokens (`{???}`).
* **Unicode BiDi & RTL Support**: Built-in script and language text direction detection (`LocaleTextDirection.ltr` / `LocaleTextDirection.rtl`) and automatic directional isolation (`\u2068...\u2069`) to protect layout integrity.
* **Bounded Security & DoS Protection**: Strictly bounded inputs (locale tags <= 64 bytes, message keys <= 128 bytes, Accept-Language headers <= 4096 bytes).
* **High-Performance Batch Lookups**: `UiLocaleTranslator` pre-computes candidate hierarchies once, providing zero-allocation $O(1)$ lookups during high-frequency list rendering.
* **Multi-Catalog Architecture**: Mix and match `FluentCatalog` (.ftl), zero-allocation `StaticCatalog` (Map tables), `JsonCatalog` (nested JSON dictionaries), and `CompositeCatalog`.
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
```

---

## Architecture

| Component | Responsibility |
| :--- | :--- |
| `LocaleTag` | BCP-47 / POSIX parser, normalizer, and directionality detector. |
| `FtlParser` | Recursive-descent AST parser for Project Fluent 1.0 syntax. |
| `FluentBundle` | In-memory message and term registry with BiDi isolation. |
| `cldr_plurals` | CLDR plural rule evaluator (Slavic, Romance, Germanic, Arabic, CJK). |
| `UiMessages` | High-level facade with fallback resolution and provenance tracking. |
| `UiLocaleTranslator` | Pre-computed locale candidate translator for high-speed batch lookups. |
| `AcceptLanguageParser` | RFC 9110 bounded `Accept-Language` quality ranking and negotiation. |

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
