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
    test('setLocale rejects malformed tags and stores canonical ones', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);
      final notifier = container.read(currentLocaleProvider.notifier);

      expect(() => notifier.setLocale('!!'), throwsArgumentError);
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
