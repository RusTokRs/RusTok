# rustok_flutter_i18n

[![pub package](https://img.shields.io/badge/pub-v0.1.0-blue.svg)](https://pub.dev)
[![Flutter](https://img.shields.io/badge/flutter-%3E%3D3.16.0-02569B.svg)](https://flutter.dev)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

Flutter widgets, `BuildContext` extensions, automatic `Directionality`, and Riverpod providers for [`rustok_i18n`](https://pub.dev/packages/rustok_i18n) (Mozilla Project Fluent 1.0).

---

## Features

* **Ergonomic BuildContext Extensions**: Translate with `context.t('auth.login_title')` and `context.tAttr('input', 'placeholder')`.
* **Safe Rendering Fallback**: `context.maybeT('key', fallback: '...')` for safe widget unit testing and design previews without needing a mounted scope.
* **InheritedWidget Scope (`FluentScope`)**: Provides [UiMessages] and the active locale down the widget tree with reactive rebuilds.
* **Automatic Directionality (`DirectionalityHandler`)**: Automatically wraps UI subtrees in Flutter's `Directionality` (`TextDirection.ltr` or `TextDirection.rtl`) based on the active locale.
* **Flutter Riverpod Integration**: Built-in `currentLocaleProvider`, `messageCatalogProvider`, `uiMessagesProvider`, `isRtlLocaleProvider`, and reactive `trProvider`.
* **Zero Name Collisions**: Fully separated `LocaleTextDirection` so you never get `The name 'TextDirection' is ambiguous` conflicts with Flutter's `dart:ui.TextDirection`.

---

## Getting Started

Add `rustok_flutter_i18n` to your `pubspec.yaml`:

```yaml
dependencies:
  flutter:
    sdk: flutter
  rustok_flutter_i18n: ^0.1.0
  flutter_riverpod: ^2.5.0 # optional, for Riverpod state management
```

---

## Usage

### 1. Basic Setup with `FluentScope`

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
    return DirectionalityHandler(
      child: MaterialApp(
        home: Scaffold(
          appBar: AppBar(title: Text(context.t('welcome'))),
          body: Center(
            child: Text(
              context.t('cart-items', args: {'count': 3}),
            ),
          ),
        ),
      ),
    );
  }
}
```

### 2. Riverpod Integration

```dart
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:rustok_flutter_i18n/rustok_flutter_i18n.dart';

class HomeScreen extends ConsumerWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Reactive translation function bound to the active locale
    final t = ref.watch(trProvider);
    final isRtl = ref.watch(isRtlLocaleProvider);

    return Scaffold(
      appBar: AppBar(title: Text(t('welcome'))),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Text(t('cart-items', args: {'count': 5})),
            const SizedBox(height: 16),
            ElevatedButton(
              onPressed: () {
                // Switch application locale reactively
                ref.read(currentLocaleProvider.notifier).setLocale('ru');
              },
              child: const Text('Сменить на русский'),
            ),
          ],
        ),
      ),
    );
  }
}
```

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
