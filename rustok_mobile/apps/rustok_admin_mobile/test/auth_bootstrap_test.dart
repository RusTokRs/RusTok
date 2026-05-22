import 'package:app_graphql/app_graphql.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:graphql/client.dart';
import 'package:rustok_admin_mobile/app_shell/auth_bootstrap.dart';

void main() {
  GraphQLClient clientWithResult(QueryResult<Object?> result) {
    final link = Link.function((request, [forward]) async* {
      yield result;
    });
    return GraphQLClient(cache: GraphQLCache(), link: link);
  }

  test('returns unauthenticated when there is no valid session', () async {
    final container = ProviderContainer(
      overrides: [
        authSessionProvider.overrideWith((ref) async => null),
      ],
    );
    addTearDown(container.dispose);

    final value = await container.read(authBootstrapProbeProvider.future);

    expect(value.isAuthenticated, isFalse);
    expect(value.userEmail, isNull);
    expect(value.tenantSlug, isNull);
  });

  test('returns authenticated payload when probe succeeds', () async {
    final queryResult = QueryResult<Object?>(
      options: QueryOptions(document: gql('query BootstrapProbe { me { email } currentTenant { slug } }')),
      source: QueryResultSource.network,
      data: {
        'me': {'email': 'operator@example.com'},
        'currentTenant': {'slug': 'tenant-main'},
      },
    );

    final container = ProviderContainer(
      overrides: [
        authSessionProvider.overrideWith(
          (ref) async => const AuthSession(
            accessToken: 'token',
            refreshToken: 'refresh',
            expiresAt: 4102444800000,
          ),
        ),
        graphQlClientProvider.overrideWithValue(clientWithResult(queryResult)),
      ],
    );
    addTearDown(container.dispose);

    final value = await container.read(authBootstrapProbeProvider.future);

    expect(value.isAuthenticated, isTrue);
    expect(value.userEmail, 'operator@example.com');
    expect(value.tenantSlug, 'tenant-main');
  });

  test('throws when probe query returns GraphQL exception', () async {
    final queryResult = QueryResult<Object?>(
      options: QueryOptions(document: gql('query BootstrapProbe { me { email } }')),
      source: QueryResultSource.network,
      exception: OperationException(graphqlErrors: [GraphQLError(message: 'boom')]),
    );

    final container = ProviderContainer(
      overrides: [
        authSessionProvider.overrideWith(
          (ref) async => const AuthSession(
            accessToken: 'token',
            refreshToken: 'refresh',
            expiresAt: 4102444800000,
          ),
        ),
        graphQlClientProvider.overrideWithValue(clientWithResult(queryResult)),
      ],
    );
    addTearDown(container.dispose);

    expect(
      container.read(authBootstrapProbeProvider.future),
      throwsA(isA<OperationException>()),
    );
  });

  test('throws when probe payload is missing required fields', () async {
    final queryResult = QueryResult<Object?>(
      options: QueryOptions(document: gql('query BootstrapProbe { me { id } }')),
      source: QueryResultSource.network,
      data: {
        'me': {'id': '1'},
      },
    );

    final container = ProviderContainer(
      overrides: [
        authSessionProvider.overrideWith(
          (ref) async => const AuthSession(
            accessToken: 'token',
            refreshToken: 'refresh',
            expiresAt: 4102444800000,
          ),
        ),
        graphQlClientProvider.overrideWithValue(clientWithResult(queryResult)),
      ],
    );
    addTearDown(container.dispose);

    expect(
      container.read(authBootstrapProbeProvider.future),
      throwsA(isA<FormatException>()),
    );
  });
}
