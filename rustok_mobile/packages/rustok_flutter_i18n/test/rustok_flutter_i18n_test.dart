import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_flutter_i18n/rustok_flutter_i18n.dart';

FluentCatalog _catalog() => FluentCatalog(useIsolating: false)
  ..registerFtl(
    'en',
    'welcome = Welcome\n'
        'mail = Hello, { \$name }\n'
        'input = Search\n'
        '    .placeholder = Type here\n',
  )
  ..registerFtl('ru', 'welcome = Добро пожаловать\n');

Future<TextDirection> _pumpDirection(WidgetTester tester, String locale) async {
  late TextDirection direction;
  await tester.pumpWidget(
    FluentScope(
      messages: UiMessages(catalog: _catalog()),
      currentLocale: locale,
      child: MaterialApp(
        builder: (context, child) => DirectionalityHandler(child: child!),
        home: Builder(
          builder: (context) {
            direction = Directionality.of(context);
            return const SizedBox();
          },
        ),
      ),
    ),
  );
  return direction;
}

void main() {
  group('FluentScope and context extensions', () {
    testWidgets('regional CLDR script controls direction with explicit override', (tester) async {
      expect(await _pumpDirection(tester, 'az-IR'), TextDirection.rtl);
      expect(await _pumpDirection(tester, 'az-Latn-IR'), TextDirection.ltr);
      expect(await _pumpDirection(tester, 'pa-PK'), TextDirection.rtl);
    });

    testWidgets('context translation selects by raw number, not rounded output',
        (tester) async {
      final catalog = FluentCatalog(useIsolating: false)..registerFtl('en',
        'pick = { NUMBER(\$n, maximumFractionDigits: 0) ->\n'
        ' [1] wrong\n *[other] correct\n}\n');
      await tester.pumpWidget(FluentScope(
        messages: UiMessages(catalog: catalog), currentLocale: 'en',
        child: MaterialApp(home: Builder(builder: (context) =>
            Text(context.t('pick', args: {'n': 1.2})))),
      ));
      expect(find.text('correct'), findsOneWidget);
      expect(find.text('wrong'), findsNothing);
    });

    testWidgets('regional date and narrow options render through context.t',
        (tester) async {
      final catalog = FluentCatalog(useIsolating: false)..registerFtl('en-GB',
          'date = { DATETIME(\$value, dateStyle: "short", timeStyle: "short") }\n'
          'day = { DATETIME(\$value, weekday: "narrow") }\n');
      final args = {'value': DateTime.utc(2026, 1, 7, 14, 3)};
      await tester.pumpWidget(FluentScope(
        messages: UiMessages(catalog: catalog), currentLocale: 'en-GB',
        child: MaterialApp(home: Builder(builder: (context) => Column(children: [
          Text(context.t('date', args: args)),
          Text(context.t('day', args: args)),
        ]))),
      ));
      expect(find.text('07/01/2026, 14:03'), findsOneWidget);
      expect(find.text('W'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('invalid dates and oversized arguments do not crash rendering',
        (tester) async {
      final catalog = FluentCatalog()..registerFtl('en',
          'date = { DATETIME(\$value) }\n'
          'greeting = Hello { \$name }!\n');
      await tester.pumpWidget(FluentScope(
        messages: UiMessages(catalog: catalog),
        currentLocale: 'en',
        child: MaterialApp(home: Builder(builder: (context) => Column(children: [
          Text(context.t('date', args: {'value': double.infinity})),
          Text(context.t('greeting', args: {
            'name': 'x' * (maxFluentResolutionCharacters + 1),
          })),
        ]))),
      ));
      expect(find.text('Infinity'), findsOneWidget);
      expect(find.text('{???}'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('without a FluentScope the host direction is preserved',
        (tester) async {
      late TextDirection direction;
      final child = DirectionalityHandler(child: Builder(builder: (context) {
        direction = Directionality.of(context);
        return const SizedBox();
      }));
      await tester.pumpWidget(Directionality(
        textDirection: TextDirection.rtl, child: child));
      expect(direction, TextDirection.rtl);
      await tester.pumpWidget(Directionality(
        textDirection: TextDirection.ltr, child: child));
      expect(direction, TextDirection.ltr);
      await tester.pumpWidget(child);
      expect(direction, TextDirection.ltr);
    });

    testWidgets('context.t, context.tAttr and args resolve through the scope',
        (tester) async {
      await tester.pumpWidget(
        FluentScope(
          messages: UiMessages(catalog: _catalog()),
          currentLocale: 'en',
          child: MaterialApp(
            home: Builder(
              builder: (context) => Column(
                children: [
                  Text(context.t('welcome')),
                  Text(context.t('mail', args: {'name': 'Ann'})),
                  Text(context.tAttr('input', 'placeholder')),
                ],
              ),
            ),
          ),
        ),
      );

      expect(find.text('Welcome'), findsOneWidget);
      expect(find.text('Hello, Ann'), findsOneWidget);
      expect(find.text('Type here'), findsOneWidget);
    });

    testWidgets('maybeT returns the fallback or the key without a scope',
        (tester) async {
      late BuildContext captured;
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Builder(
            builder: (context) {
              captured = context;
              return const SizedBox();
            },
          ),
        ),
      );

      expect(captured.maybeT('welcome'), 'welcome');
      expect(captured.maybeT('welcome', fallback: 'Hi'), 'Hi');
    });

    testWidgets('context.t throws a StateError without a scope', (tester) async {
      late BuildContext captured;
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Builder(
            builder: (context) {
              captured = context;
              return const SizedBox();
            },
          ),
        ),
      );

      expect(() => captured.t('welcome'), throwsStateError);
    });

    testWidgets('DirectionalityHandler uses RTL for Arabic and LTR for English',
        (tester) async {
      expect(await _pumpDirection(tester, 'ar'), TextDirection.rtl);
      expect(await _pumpDirection(tester, 'en'), TextDirection.ltr);
    });
  });

  group('Riverpod bindings', () {
    testWidgets('synchronous readiness refresh publishes changed catalog data',
        (tester) async {
      final catalog = StaticCatalog({'en': {'welcome': 'Before'}});
      final container = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(catalog),
      ]);
      addTearDown(container.dispose);
      final child = MaterialApp(home: Builder(builder: (context) =>
          Text(context.t('welcome'))));
      Widget tree() => UncontrolledProviderScope(
        container: container,
        child: RiverpodFluentScope(child: child),
      );
      await tester.pumpWidget(tree());
      expect(find.text('Before'), findsOneWidget);
      catalog.registerStatic('en', {'welcome': 'After'});
      container.invalidate(localeReadyProvider('en'));
      await tester.pumpWidget(tree());
      expect(find.text('After'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    for (final useLazy in [false, true]) {
      testWidgets('ready catalogs preserve mounted state (lazy=$useLazy)',
          (tester) async {
        final MessageCatalog catalog;
        if (useLazy) {
          final lazy = LazyCatalog(loader: (locale) =>
              'welcome = $locale\n');
          await lazy.loadForLocale('ru'); // both ru and fallback en
          catalog = CompositeCatalog([StaticCatalog(), lazy]);
        } else {
          catalog = CompositeCatalog([_catalog()]);
        }
        final container = ProviderContainer(overrides: [
          messageCatalogProvider.overrideWithValue(catalog),
        ]);
        addTearDown(container.dispose);
        final formKey = GlobalKey<FormState>();
        await tester.pumpWidget(UncontrolledProviderScope(
          container: container,
          child: RiverpodFluentScope(
            loading: const SizedBox(key: ValueKey('loading')),
            child: MaterialApp(home: Form(key: formKey,
              child: Builder(builder: (context) => Text(context.t('welcome'))),
            )),
          ),
        ));
        expect(find.byKey(const ValueKey('loading')), findsNothing);
        final initialState = formKey.currentState;
        expect(initialState, isNotNull);
        container.read(currentLocaleProvider.notifier).setLocale('ru');
        await tester.pump();
        expect(find.byKey(const ValueKey('loading')), findsNothing);
        expect(identical(formKey.currentState, initialState), isTrue);
        expect(find.text(useLazy ? 'ru' : 'Добро пожаловать'), findsOneWidget);
      });
    }

    testWidgets('catalog replacement ignores completion of the old load',
        (tester) async {
      final oldSource = Completer<String?>();
      final container = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(
          LazyCatalog(loader: (_) => oldSource.future)),
      ]);
      addTearDown(container.dispose);
      await tester.pumpWidget(UncontrolledProviderScope(
        container: container,
        child: RiverpodFluentScope(child: MaterialApp(
          home: Builder(builder: (context) => Text(context.t('welcome'))),
        )),
      ));
      container.updateOverrides([
        messageCatalogProvider.overrideWithValue(StaticCatalog({
          'en': {'welcome': 'Replacement'},
        })),
      ]);
      await tester.pumpAndSettle();
      expect(find.text('Replacement'), findsOneWidget);
      oldSource.completeError(StateError('old catalog failed'));
      await tester.pumpAndSettle();
      expect(find.text('Replacement'), findsOneWidget);
      expect(tester.takeException(), isNull);
      expect(container.read(localeReadyProvider('en')).hasError, isFalse);
    });

    testWidgets('completing a load after scope disposal is harmless',
        (tester) async {
      final source = Completer<String?>();
      final container = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(
          LazyCatalog(loader: (_) => source.future)),
      ]);
      addTearDown(container.dispose);
      await tester.pumpWidget(UncontrolledProviderScope(
        container: container,
        child: const RiverpodFluentScope(child: SizedBox()),
      ));
      await tester.pumpWidget(const SizedBox());
      source.completeError(StateError('late load error'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    });

    test('fallback updates refresh messages without resetting active selection',
        () async {
      final container = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(StaticCatalog({
          'en': {'fallback': 'English'},
          'fr': {'fallback': 'French'},
        })),
        defaultLocaleProvider.overrideWithValue('en'),
      ]);
      addTearDown(container.dispose);
      final subscription = container.listen(trProvider, (_, __) {});
      addTearDown(subscription.close);
      container.read(currentLocaleProvider.notifier).setLocale('ru');
      expect(container.read(trProvider)('fallback'), 'English');
      container.updateOverrides([
        messageCatalogProvider.overrideWithValue(
          container.read(messageCatalogProvider)),
        defaultLocaleProvider.overrideWithValue('fr'),
      ]);
      await container.pump();
      expect(container.read(currentLocaleProvider), 'ru');
      expect(container.read(trProvider)('fallback'), 'French');
    });

    test('unobserved readiness is disposed while catalog data stays cached',
        () async {
      var loads = 0;
      final catalog = LazyCatalog(loader: (_) {
        loads++;
        return 'welcome = Cached\n';
      });
      final container = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(catalog),
      ]);
      addTearDown(container.dispose);
      final provider = localeReadyProvider('en');
      final subscription = container.listen(provider, (_, __) {});
      await container.read(provider.future);
      subscription.close();
      await container.pump();
      expect(container.getAllProviderElements().any(
          (element) => element.provider == provider), isFalse);
      expect(catalog.isLoaded('en'), isTrue);
      final again = container.listen(provider, (_, __) {});
      addTearDown(again.close);
      await container.read(provider.future);
      expect(loads, 1);
    });

    test('nested containers isolate catalog, locale, readiness and RTL', () async {
      final parent = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(_catalog()),
      ]);
      addTearDown(parent.dispose);
      expect(parent.read(trProvider)('welcome'), 'Welcome');
      expect(parent.read(isRtlLocaleProvider), isFalse);

      final child = ProviderContainer(parent: parent, overrides: [
        messageCatalogProvider.overrideWithValue(StaticCatalog({
          'ar': {'welcome': 'Child'},
        })),
        defaultLocaleProvider.overrideWithValue('ar'),
      ]);
      addTearDown(child.dispose);
      final subscription = child.listen(trProvider, (_, __) {});
      addTearDown(subscription.close);
      await child.read(localeReadyProvider('ar').future);
      expect(child.read(trProvider)('welcome'), 'Child');
      expect(child.read(currentLocaleProvider), 'ar');
      expect(child.read(isRtlLocaleProvider), isTrue);
      child.read(currentLocaleProvider.notifier).setLocale('ru');
      expect(parent.read(currentLocaleProvider), 'en');
      expect(parent.read(trProvider)('welcome'), 'Welcome');
    });

    test('invalid fallback configuration fails instead of silently losing it', () {
      final container = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(_catalog()),
        platformFallbackProvider.overrideWithValue('en-u'),
      ]);
      addTearDown(container.dispose);
      expect(() => container.read(uiMessagesProvider), throwsArgumentError);
    });

    testWidgets('trProvider loads nested catalogs without a FluentScope',
        (tester) async {
      final source = Completer<String?>();
      final lazy = LazyCatalog(loader: (_) => source.future);
      final container = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(CompositeCatalog([
          StaticCatalog(),
          CompositeCatalog([lazy]),
        ])),
      ]);
      addTearDown(container.dispose);
      await tester.pumpWidget(UncontrolledProviderScope(
        container: container,
        child: MaterialApp(home: Consumer(builder: (context, ref, child) {
          return Text(ref.watch(trProvider)('welcome', fallback: 'waiting'));
        })),
      ));
      expect(find.text('waiting'), findsOneWidget);
      source.complete('welcome = Loaded\n');
      await tester.pumpAndSettle();
      expect(find.text('Loaded'), findsOneWidget);
      expect(lazy.isLoaded('en'), isTrue);
    });

    testWidgets('a retry completed between frames notifies inherited consumers',
        (tester) async {
      var attempts = 0;
      final catalog = LazyCatalog(loader: (_) {
        if (++attempts == 1) throw StateError('offline');
        return 'welcome = Retried\n';
      });
      final container = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(catalog),
      ]);
      addTearDown(container.dispose);
      await tester.pumpWidget(UncontrolledProviderScope(
        container: container,
        child: RiverpodFluentScope(
          child: MaterialApp(home: Builder(builder: (context) {
            return Text(context.t('welcome', fallback: 'missing'));
          })),
        ),
      ));
      await tester.pumpAndSettle();
      expect(container.read(localeReadyProvider('en')).hasError, isTrue);
      expect(find.text('missing'), findsOneWidget);

      container.invalidate(localeReadyProvider('en'));
      await container.read(localeReadyProvider('en').future);
      // Deliberately no frame between loading and success.
      await tester.pump();
      expect(find.text('Retried'), findsOneWidget);
      expect(attempts, 2);
    });

    testWidgets('a late old locale response does not replace the active locale',
        (tester) async {
      final english = Completer<String?>();
      final russian = Completer<String?>();
      final catalog = LazyCatalog(loader: (locale) =>
          locale == 'ru' ? russian.future : english.future);
      final container = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(catalog),
      ]);
      addTearDown(container.dispose);
      await tester.pumpWidget(UncontrolledProviderScope(
        container: container,
        child: RiverpodFluentScope(
          child: MaterialApp(home: Builder(builder: (context) {
            return Text(context.t('welcome'));
          })),
        ),
      ));
      container.read(currentLocaleProvider.notifier).setLocale('ru');
      await tester.pump();
      russian.complete('welcome = Russian\n');
      english.complete('welcome = English\n');
      await tester.pumpAndSettle();
      expect(find.text('Russian'), findsOneWidget);
      expect(find.text('English'), findsNothing);
      expect(container.read(currentLocaleProvider), 'ru');
    });

    testWidgets('loading child is replaced after failure and fallback is usable',
        (tester) async {
      final source = Completer<String?>();
      final catalog = CompositeCatalog([
        LazyCatalog(loader: (_) => source.future),
        StaticCatalog({'en': {'welcome': 'Offline fallback'}}),
      ]);
      final container = ProviderContainer(overrides: [
        messageCatalogProvider.overrideWithValue(catalog),
      ]);
      addTearDown(container.dispose);
      await tester.pumpWidget(UncontrolledProviderScope(
        container: container,
        child: RiverpodFluentScope(
          loading: const Directionality(
            textDirection: TextDirection.ltr,
            child: Text('loading'),
          ),
          child: MaterialApp(home: Builder(builder: (context) {
            return Text(context.t('welcome'));
          })),
        ),
      ));
      expect(find.text('loading'), findsOneWidget);
      source.completeError(StateError('offline'));
      await tester.pumpAndSettle();
      expect(find.text('loading'), findsNothing);
      expect(find.text('Offline fallback'), findsOneWidget);
      expect(container.read(localeReadyProvider('en')).hasError, isTrue);
    });

    test('setLocale rejects malformed tags and stores canonical ones', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      final notifier = container.read(currentLocaleProvider.notifier);

      for (final locale in ['!!', 'en-u', 'en--US', 'en-u-!']) {
        expect(() => notifier.setLocale(locale), throwsArgumentError);
      }
      expect(container.read(currentLocaleProvider), 'en');

      notifier.setLocale('ru_ru');
      expect(container.read(currentLocaleProvider), 'ru-RU');
    });

    test('messageCatalogProvider must be overridden at bootstrap', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      expect(
        () => container.read(uiMessagesProvider),
        throwsA(isA<UnimplementedError>()),
      );
    });

    test('trProvider and isRtlLocaleProvider follow the active locale', () {
      final container = ProviderContainer(
        overrides: [messageCatalogProvider.overrideWithValue(_catalog())],
      );
      addTearDown(container.dispose);

      final t = container.read(trProvider);
      expect(t('welcome'), 'Welcome');
      expect(container.read(isRtlLocaleProvider), isFalse);

      container.read(currentLocaleProvider.notifier).setLocale('ru');
      expect(container.read(trProvider)('welcome'), 'Добро пожаловать');
    });

    testWidgets('RiverpodFluentScope re-renders when the locale changes',
        (tester) async {
      final container = ProviderContainer(
        overrides: [messageCatalogProvider.overrideWithValue(_catalog())],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: RiverpodFluentScope(
            child: MaterialApp(
              home: Builder(builder: (context) => Text(context.t('welcome'))),
            ),
          ),
        ),
      );
      expect(find.text('Welcome'), findsOneWidget);

      container.read(currentLocaleProvider.notifier).setLocale('ru');
      await tester.pumpAndSettle();
      expect(find.text('Добро пожаловать'), findsOneWidget);
    });

    testWidgets('RiverpodFluentScope shows translations after a LazyCatalog loads',
        (tester) async {
      final catalog = LazyCatalog(
        loader: (locale) async =>
            locale == 'en' ? 'welcome = Welcome\n' : null,
      );
      final container = ProviderContainer(
        overrides: [messageCatalogProvider.overrideWithValue(catalog)],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: RiverpodFluentScope(
            child: MaterialApp(
              home: Builder(
                builder: (context) => Text(context.maybeT('welcome', fallback: '...')),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(catalog.isLoaded('en'), isTrue);
      expect(find.text('Welcome'), findsOneWidget);
    });
  });
}
