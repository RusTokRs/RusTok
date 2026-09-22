import 'package:app_graphql/app_graphql.dart';
import 'package:test/test.dart';

void main() {
  group('GraphQlEndpoints', () {
    const endpoints = GraphQlEndpoints();

    test('builds canonical http endpoint path', () {
      final uri = endpoints.httpUri(Uri.parse('https://example.com'));
      expect(uri.toString(), 'https://example.com/api/graphql');
    });

    test('converts https scheme to wss for websocket endpoint', () {
      final uri = endpoints.wsUri(Uri.parse('https://example.com'));
      expect(uri.toString(), 'wss://example.com/api/graphql/ws');
    });

    test('preserves nested base path', () {
      final uri = endpoints.httpUri(Uri.parse('https://example.com/platform'));
      expect(uri.toString(), 'https://example.com/platform/api/graphql');
    });
  });
}
