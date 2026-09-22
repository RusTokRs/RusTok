import 'package:app_graphql/app_graphql.dart';
import 'package:test/test.dart';

void main() {
  group('GraphQlHeadersBuilder', () {
    const builder = GraphQlHeadersBuilder();

    test('builds mandatory headers', () {
      const context = GraphQlRequestContext(tenantSlug: 'acme', locale: 'ru');
      final headers = builder.build(context);
      expect(headers['X-Tenant-Slug'], 'acme');
      expect(headers['Accept-Language'], 'ru');
      expect(headers.containsKey('Authorization'), isFalse);
    });

    test('adds auth and tenant id when provided', () {
      const context = GraphQlRequestContext(
        tenantSlug: 'acme',
        locale: 'en',
        tenantId: 'tenant-1',
        accessToken: 'token-123',
      );
      final headers = builder.build(context);
      expect(headers['X-Tenant-ID'], 'tenant-1');
      expect(headers['Authorization'], 'Bearer token-123');
    });

    test('builds ws init payload expected by server contract', () {
      const context = GraphQlRequestContext(
        tenantSlug: 'acme',
        locale: 'en',
        accessToken: 'token-123',
      );
      final payload = builder.buildWsInitPayload(context);
      expect(payload['tenantSlug'], 'acme');
      expect(payload['locale'], 'en');
      expect(payload['token'], 'token-123');
    });
  });
}
