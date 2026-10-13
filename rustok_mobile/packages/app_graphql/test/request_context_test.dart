import 'package:app_graphql/app_graphql.dart';
import 'package:test/test.dart';

void main() {
  group('GraphQlRequestContext', () {
    test('accepts non-empty tenant and locale', () {
      final context = GraphQlRequestContext(tenantSlug: 'acme', locale: 'en');
      expect(context.tenantSlug, 'acme');
      expect(context.locale, 'en');
    });

    test('rejects blank tenant', () {
      expect(
        () => GraphQlRequestContext(tenantSlug: '   ', locale: 'en'),
        throwsA(isA<AssertionError>()),
      );
    });

    test('rejects blank locale', () {
      expect(
        () => GraphQlRequestContext(tenantSlug: 'acme', locale: '  '),
        throwsA(isA<AssertionError>()),
      );
    });
  });
}
