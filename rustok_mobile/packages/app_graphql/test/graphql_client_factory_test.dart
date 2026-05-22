import 'package:app_graphql/app_graphql.dart';
import 'package:graphql/client.dart';
import 'package:test/test.dart';

void main() {
  group('GraphQlClientFactory', () {
    const factory = GraphQlClientFactory();
    const context = GraphQlRequestContext(tenantSlug: 'default', locale: 'en');

    test('create returns GraphQLClient with split transport support', () {
      final client = factory.create(
        const GraphQlClientConfig(
          baseUri: Uri(scheme: 'https', host: 'example.com'),
          context: context,
        ),
      );

      expect(client, isA<GraphQLClient>());
    });

    test('createHttpOnly returns GraphQLClient for non-subscription flows', () {
      final client = factory.createHttpOnly(
        const GraphQlClientConfig(
          baseUri: Uri(scheme: 'https', host: 'example.com'),
          context: context,
        ),
      );

      expect(client, isA<GraphQLClient>());
    });
  });
}
