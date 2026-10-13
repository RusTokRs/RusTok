# rustok_flutter_i18n

Internal monorepo package (`publish_to: none`).
[![Flutter](https://img.shields.io/badge/flutter-%3E%3D3.16.0-02569B.svg)](https://flutter.dev)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

Flutter widgets, `BuildContext` extensions, automatic `Directionality`, and Riverpod providers for [`rustok_i18n`](../rustok_i18n/README.md) (Mozilla Project Fluent 1.0).

---

## Features

* **Ergonomic BuildContext Extensions**: Translate with `context.t('auth.login_title')` and `context.tAttr('input', 'placeholder')`.
* **Safe Rendering Fallback**: `context.maybeT('key', fallback: '...')` for widget tests and design previews without a mounted scope.
* **InheritedWidget Scope (`FluentScope`)**: Provides `UiMessages` and the active locale down the widget tree.
* **Riverpod as the single source of truth**: `RiverpodFluentScope` publishes `currentLocaleProvider` and `uiMessagesProvider` into the widget tree, so locale changes rebuild the UI.
* **Automatic Directionality (`DirectionalityHandler`)**: Wraps UI subtrees in Flutter's `Directionality` (`ltr` / `rtl`) based on the active locale.
* **Riverpod providers**: `currentLocaleProvider`, `messageCatalogProvider`, `uiMessagesProvider`, `isRtlLocaleProvider`, and `trProvider`.
* **No name clashes with Flutter**: the core package uses `LocaleTextDirection`, so `TextDirection` stays unambiguous.

---

## Getting Started

Add `rustok_flutter_i18n` to your `pubspec.yaml`:

```yaml
dependencies:
  flutter:
    sdk: flutter
  rustok_flutter_i18n:
    path: ../../packages/rustok_flutter_i18n
  flutter_riverpod: ^2.5.0
```

The path above is relative to an app under `rustok_mobile/apps/`; adjust it for your host. This wrapper is not published on pub.dev.

`flutter_riverpod` is a required dependency: the package's providers and `RiverpodFluentScope` are built on it.

---

## Usage

### 1. Without Riverpod: `FluentScope`

Put `FluentScope` **above** `MaterialApp`, and use `DirectionalityHandler` through `MaterialApp.builder` so that it sits below the scope.

```dart
import 'package:flutter/material.dart';
import 'package:rustok_flutter_i18n/rustok_flutter_i18n.dart';

void main() {
  final catalog = FluentCatalog();
  catalog.registerFtl('en', '''
welcome = Welcome to RusToK!
cart-items = { \$count ->
    [0] No items
    [one] 1 item
   *[other] { \$count } items
}
''');
  catalog.registerFtl('ru', '''
welcome = Добро пожаловать в RusToK!
cart-items = { \$count ->
    [0] Корзина пуста
    [one] { \$count } товар
    [few] { \$count } товара
   *[other] { \$count } товаров
}
''');

  final messages = UiMessages(catalog: catalog, defaultLocale: 'en');

  runApp(
    FluentScope(
      messages: messages,
      currentLocale: 'ru',
      child: const MyApp(),
    ),
  );
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      builder: (context, child) => DirectionalityHandler(child: child!),
      home: Scaffold(
        appBar: AppBar(title: Text(context.t('welcome'))),
        body: Center(
          child: Text(context.t('cart-items', args: {'count': 3})),
        ),
      ),
    );
  }
}
```

### 2. With Riverpod

Override `messageCatalogProvider` at bootstrap (it has no default and throws `UnimplementedError` if left unset), and put `RiverpodFluentScope` above `MaterialApp`.

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:rustok_flutter_i18n/rustok_flutter_i18n.dart';

void main() {
  final catalog = FluentCatalog();
  catalog.registerFtl('en', 'welcome = Welcome!\n');
  catalog.registerFtl('ru', 'welcome = Добро пожаловать!\n');

  runApp(
    ProviderScope(
      overrides: [
        messageCatalogProvider.overrideWithValue(catalog),
        defaultLocaleProvider.overrideWithValue('en'),
      ],
      child: const RiverpodFluentScope(child: MyApp()),
    ),
  );
}

class MyApp extends ConsumerWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return MaterialApp(
      builder: (context, child) => DirectionalityHandler(child: child!),
      home: Scaffold(
        appBar: AppBar(title: Text(context.t('welcome'))),
        body: Center(
          child: ElevatedButton(
            onPressed: () {
              // Invalid tags throw ArgumentError instead of being stored.
              ref.read(currentLocaleProvider.notifier).setLocale('ru');
            },
            child: const Text('Русский'),
          ),
        ),
      ),
    );
  }
}
```

Inside any `ConsumerWidget` you can also use the providers directly:

```dart
final t = ref.watch(trProvider);
final isRtl = ref.watch(isRtlLocaleProvider);
return Text(t('cart-items', args: {'count': 5}));
```

---

## Lazy (asynchronous) catalogs

Use a `LazyCatalog` when translations come from assets, files, or a server. Override `messageCatalogProvider` with it. `RiverpodFluentScope` loads the active locale and its fallback chain through `localeReadyProvider`, and rebuilds when loading finishes. Use `loading:` to show a placeholder meanwhile:

```dart
final catalog = LazyCatalog(loader: (locale) async {
  // Only a known absent locale should be cached as empty. Do not catch all
  // errors: IO/asset failures must reach localeReadyProvider so retry works.
  const available = {'en', 'ru'};
  if (!available.contains(locale)) return null;
  return rootBundle.loadString('assets/i18n/$locale.ftl');
});

RiverpodFluentScope(
  loading: const MaterialApp(
    home: Scaffold(body: Center(child: CircularProgressIndicator())),
  ),
  child: MaterialApp(...),
);
```

### Loading, scoping and lifecycle contract

* `MessageCatalog.loadForLocale` is a `FutureOr<void>` preparation hook: await it rather than assuming a `Future` or calling `.then` directly. Eager catalogs and fully cached lazy catalogs return synchronously, avoiding a transient loading widget that would unmount UI state. Eager catalogs do nothing; `CompositeCatalog` loads every child (including nested lazy catalogs), waits for all to settle, and propagates errors without discarding successful loads. Its priority list is immutable: replace the composite to change its children.
* `trProvider` observes readiness itself; a `ConsumerWidget` using it does not need `RiverpodFluentScope`. Both `trProvider` and `localeReadyProvider` are **auto-disposed** when unobserved. Custom providers watching them must also be auto-dispose providers. Catalog data and in-flight work are retained by the catalog, not by the provider family.
* Provider dependencies are declared for nested `ProviderScope` overrides. An overridden catalog cannot reuse the parent's translations. Override `defaultLocaleProvider` for a scope-local initial locale; changing a fallback after initialization does not reset an explicitly selected active locale. Override `currentLocaleProvider` as well when a nested subtree needs independent selection with the same default.
* Invalid fallback configuration throws instead of silently dropping the chain. The active state uses extension-free catalog tags, not a full formatting locale with Unicode preferences.
* A `null` loader result means permanently absent for the lifetime of that catalog. A thrown error is retryable. Invalidating readiness retries failed loads, **not** successfully cached or absent sources. To refresh successful translations, replace the catalog/provider. Mutable eager catalogs are not observable; invalidate `uiMessagesProvider` after mutating one (or change `FluentScope.revision` in non-Riverpod hosts).
* `loading` replaces the entire child, so when the child is `MaterialApp`, the placeholder must provide its own framework ancestors if needed. This may unmount navigation/UI state. Omit `loading` to keep the app mounted and show fallback strings during loading.
* `FluentScope` does not load catalogs by itself. Non-Riverpod hosts must await `catalog.loadForLocale`, publish the validated effective locale, and update `revision` after asynchronous catalog changes.
* `DirectionalityHandler` controls text direction only. Without a `FluentScope` it preserves the host's enclosing direction, falling back to LTR only if there is no enclosing `Directionality`. The host still owns `MaterialApp.locale`, `supportedLocales`, `localizationsDelegates`, device-locale negotiation and synchronization of Flutter's built-in control labels. No `flutter_localizations` delegate is installed implicitly.

---

## Engineering audit — 2026-10-13

Scope: every wrapper production file, its example/tests/manifest, and the core catalog, locale and fallback paths used by the wrapper. Base: `85e83666947217e772ff234664692f61938c4d99`. This is not a new conformance audit of the complete Fluent parser, CLDR tables or CLI.

Fixed: stale provider-only translations after loading; skipped lazy children in composite catalogs; missing nested-provider dependencies; unbounded readiness-family retention; inherited consumers missing a retry completed between frames; malformed locale extensions/separators/duplicate variants; loss of private-use subtags; skipped intermediate variant fallbacks; invalid input passed directly to lazy loaders; misleading installation/loading guidance; committed machine-specific Dart caches; unconditional CI error annotations.

Fourteen regression tests were added across this package's test file and `rustok_i18n/test/rustok_i18n_test.dart`, and the existing locale validation test was expanded. Tests were **not run**, at the maintainer's request. Dart/Flutter SDKs are unavailable in the audit environment, so compilation, formatting and analyzer results are also unverified. Static checks completed: Tree-sitter parsed all eight changed Dart files without syntax errors, the workflow YAML parsed successfully, and `git diff HEAD --check` passed. These checks do not validate Dart types or runtime behavior.

### Second pass — lifecycle and data-boundary review

The first pass's callback-based revision counter had a listener gap: Riverpod removes build-scoped listeners before rebuilding and flushes a provider before attaching a new one. The scope now compares readiness snapshots and loading-future identity in `build`, covering synchronous refreshes and retries completed between frames without listener-driven `setState`.

Further fixes:

* Synchronous readiness for eager, composite-eager and fully cached lazy catalogs, so changing a ready locale does not unnecessarily unmount `MaterialApp`/forms when `loading` is supplied.
* The direct lazy preparation API validates requested/default/platform locale inputs before invoking any loader.
* `DirectionalityHandler` preserves an existing host direction when no Fluent scope exists.
* Explicit attribute lookups obey the same bounded-key policy as normal lookups; dotted attributes precede lossy aliases while literal flat keys keep priority. Requested parent-chain provenance is not misreported as default fallback when the chains overlap.
* JSON registration detects cyclic/deep maps and ambiguous flattened keys, with atomic failure instead of partial catalog writes. Shared, non-cyclic subtrees remain valid.
* Non-finite number formatting preserves the sign of infinity and percent/currency layout instead of rendering positive infinity as negative or dropping percent affixes.
* CI now covers minimum supported and stable Dart/Flutter versions; this matrix has not been executed in this session.

Lifecycle review used Riverpod 2.5.1 source at `rrousselGit/riverpod` commit `6b32bd938918113888f9991e3297d80a7ed145c9` (`ConsumerStatefulElement.build`, provider listener attachment and FutureProvider state transitions). Additional regressions cover mounted-state preservation, synchronous invalidation, catalog replacement during loading, late errors after unmount, fallback reconfiguration, malformed attributes, JSON transactionality and non-finite formatting. These tests are added, **not run**. Tree-sitter/YAML/diff checks remain syntax/source evidence only, not Dart analysis or runtime verification. This expands review into specific resolver paths, not the entire Fluent/CLDR implementation.

### Third pass — bounded Fluent evaluation and numeric boundaries

Added parser nesting limits with per-entry junk recovery and a shared per-formatting budget for depth, evaluation steps and cumulative character work. This closes acyclic/exponential reference expansion, empty-expansion and deeply nested raw-argument paths that a cycle guard alone did not cover. Limit exhaustion returns the existing `{???}` marker atomically, including when rendering through `context.t`; see the [core safety contract](../rustok_i18n/README.md#resource-and-formatting-safety) for exact limits and host responsibilities.

Also corrected invalid epoch/date conversion and non-finite digit options; percent multiplication overflow/rounding noise; native minimum-integer `abs()` overflow in number/currency formatting; and mutable root resource entries drifting from cached AST maps. The numeric test-generator model was synchronized, but neither it nor the generated test suites were executed. Eleven core regressions and one widget regression were added in this pass, without running them.

At this continuation the local `main` reference was `1869b2d7c40c368af12e4fe37ea0a310fa301f8a`; comparing it with the original base showed no intervening changes in the two i18n packages or their CI workflow. Work remains on the assigned branch; this is not a merge or integration claim. Syntax/source checks cover thirteen changed Dart files, YAML and the modified Python sources; Dart/Flutter compilation and runtime verification remain outstanding.

### Fourth pass — locale preferences and negotiation

Reviewed the core HTTP-locale path against `crates/ui/rustok-ui-i18n/src/accept_language.rs`. Added `parseAcceptLanguageStrict` with retained exclusions and typed resource-limit errors; kept the existing positive-only parser as a projection. Fixed raw UTF-8 bounds, counting all input ranges, validation/canonicalization and source indices. Negotiation now respects exclusions (including fallback), compares the default canonically, avoids explicitly conflicting sibling scripts and returns null when no supported locale is allowed. These helpers remain host-owned choices; no HTTP negotiation is wired into Flutter providers implicitly.

Full-locale serialization now orders extension singleton blocks deterministically, and the core locale bound is measured in UTF-8 bytes. Fifteen additional Dart regressions were added, not run. See [HTTP locale API and compatibility notes](../rustok_i18n/README.md#4-http-accept-language-negotiation-server--shelf--dart-frog).

The local `main` reference observed in this pass is `232e9368d1fc5fe91df3f46f7f0c4e85acc69fd4`. Since the original base, the only intervening changes in these package/workflow paths are generated Flutter package caches and a lockfile; they were not imported into this working branch. No integration is claimed. Syntax/source checks remain separate from unperformed Dart/Flutter analysis and tests.

### Fifth pass — date/time formatting

Corrected canonical locale lookup, added the en-GB regional date/hour profile, and reconciled narrow/stand-alone text and short-date year widths with CLDR 48.2 Gregorian data. Fixed Polish standalone month case, Russian abbreviated standalone months, numeric standalone component widths, invalid-option handling and the sign of two-digit negative years. No new public method parameters or runtime dependencies were introduced. The core's LICENSE now carries the third-party Unicode data notice.

Eleven core regressions and one widget regression were added, not run. Cases include regional output, all supported short-year profiles, narrow names/context, explicit width/style interaction, midnight/noon, UTC conversion, and `context.t` integration. Downloading/reading CLDR source data and static syntax checks are not runtime test evidence. The [core date contract](../rustok_i18n/README.md#other-capabilities-and-limitations) explicitly describes the supported subset, signed proleptic years and differences from a full ICU formatter.

Local main remains `232e9368d1fc5fe91df3f46f7f0c4e85acc69fd4`; work stays on the assigned branch and is not integrated. Dart/Flutter SDKs are unavailable; compilation, analyzer, formatting and test execution remain outstanding.

Maintainer checks (run from each package directory):

```sh
# rustok_i18n
dart pub get
dart analyze
dart test

# rustok_flutter_i18n
flutter pub get
flutter analyze
flutter test
```

---

## Status and limitations

* Widget and Riverpod tests live in `test/` and run in CI (`.github/workflows/rustok-i18n.yml`).
* Currency formatting is provided by the re-exported core via `NUMBER(..., style: "currency", currency: "USD")` and `formatFluentCurrency`, not a Fluent `CURRENCY()` function. Remote catalog clients are supplied by the host.
* Attribute lookups use dotted keys: `context.t('login-form.placeholder')`. The `trProvider` function does not expose an `attribute` parameter — call `context.tAttr(id, 'attribute')` or `UiMessages.tAttr` for that.
* `context.activeLocale`, `context.isRtlLocale`, `context.t` and `context.tAttr` throw a `StateError` when there is no `FluentScope` above the widget (keep `RiverpodFluentScope` above `MaterialApp`). Use `maybeT`, `FluentScope.maybeOf(context)` and a `null` check instead of a hard dependency.
* If a `LazyCatalog` loader throws, `localeReadyProvider` enters an error state: loading is over, so `RiverpodFluentScope` replaces the placeholder with `child` and keeps rendering the fallback chain, and the failure is not retried while the provider remains observed. Watch `localeReadyProvider(locale)` to surface the error, and retry with `ref.invalidate(localeReadyProvider(locale))`.

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

### Sixth pass: resolver semantics

`context.t` inherits the core's typed selection: a rounded `NUMBER(1.2)` cannot match `[1]`; numeric strings no longer match numeric keys implicitly. Nested NUMBER/DATETIME calls preserve options, and currency precision/padding and CLDR fractional operands are corrected in core. Plural category still follows the raw number, not NUMBER display options. A widget regression was added but not run; see the core README for compatibility and conformance limits.

### Full CLDR likely-script inference

Scope direction now follows core CLDR likely subtags and generated RTL script metadata, including scriptless regional tags such as `az-IR` and `pa-PK`. An explicit `Latn` overrides the inference. A new widget regression covers this transition (not run). Core regional fallback chains can load extra script candidates; absent resources must be handled by lazy loaders. See the core README for generation, compatibility and verification details.
