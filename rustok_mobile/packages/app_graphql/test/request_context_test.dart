import 'package:app_graphql/app_graphql.dart';
import 'package:test/test.dart';

void main() {
  group('GraphQlRequestContext', () {
    test('accepts non-empty tenant and locale', () {
      const context = GraphQlRequestContext(tenantSlug: 'acme', locale: 'en');
      expect(context.tenantSlug, 'acme');
      expect(context.locale, 'en');
    });

    test('rejects blank tenant', () {
      expect(
        () => const GraphQlRequestContext(tenantSlug: '   ', locale: 'en'),
        throwsA(isA<AssertionError>()),
      );
    });

    test('rejects blank locale', () {
      expect(
        () => const GraphQlRequestContext(tenantSlug: 'acme', locale: '  '),
        throwsA(isA<AssertionError>()),
      );
    });
  });
}
