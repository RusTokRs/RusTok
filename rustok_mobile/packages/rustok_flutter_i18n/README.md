# rustok_flutter_i18n

[![pub package](https://img.shields.io/badge/pub-v0.1.0-blue.svg)](https://pub.dev)
[![Flutter](https://img.shields.io/badge/flutter-%3E%3D3.16.0-02569B.svg)](https://flutter.dev)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

Flutter widgets, `BuildContext` extensions, automatic `Directionality`, and Riverpod providers for [`rustok_i18n`](https://pub.dev/packages/rustok_i18n) (Mozilla Project Fluent 1.0).

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
  rustok_flutter_i18n: ^0.1.0
  flutter_riverpod: ^2.5.0
```

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
  try {
    return await rootBundle.loadString('assets/i18n/$locale.ftl');
  } catch (_) {
    return null; // no source for this locale
  }
});

RiverpodFluentScope(
  loading: const Center(child: CircularProgressIndicator()),
  child: MaterialApp(...),
);
```

---

## Status and limitations

* Widget and Riverpod tests live in `test/` and run in CI (`.github/workflows/rustok-i18n.yml`).
* `CURRENCY()` and remote catalog clients are not provided by this package.
* Attribute lookups use dotted keys: `context.t('login-form.placeholder')`. The `trProvider` function does not expose an `attribute` parameter — call `context.tAttr(id, 'attribute')` or `UiMessages.tAttr` for that.
* `context.activeLocale`, `context.isRtlLocale`, `context.t` and `context.tAttr` throw a `StateError` when there is no `FluentScope` above the widget (keep `RiverpodFluentScope` above `MaterialApp`). Use `maybeT`, `FluentScope.maybeOf(context)` and a `null` check instead of a hard dependency.
* If a `LazyCatalog` loader throws, `localeReadyProvider` enters an error state: loading is over, so `RiverpodFluentScope` replaces the placeholder with `child` and keeps rendering the fallback chain, and the failure is not retried automatically. Watch `localeReadyProvider(locale)` to surface the error, and retry with `ref.invalidate(localeReadyProvider(locale))`.

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
