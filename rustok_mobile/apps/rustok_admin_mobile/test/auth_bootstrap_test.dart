import 'package:app_graphql/app_graphql.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:graphql/client.dart';
import 'package:rustok_admin_mobile/app_shell/auth_bootstrap.dart';

void main() {
  GraphQLClient clientWithResult(QueryResult<Object?> result) {
    final link = Link.function((request, [forward]) {
      return Stream.value(
        Response(
          data: result.data,
          errors: result.exception?.graphqlErrors,
          response: result.data ?? const <String, dynamic>{},
        ),
      );
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
        '__typename': 'Query',
        'me': {
          '__typename': 'User',
          'id': 'operator-1',
          'email': 'operator@example.com',
          'permissions': ['modules:manage'],
        },
        'currentTenant': {
          '__typename': 'Tenant',
          'id': 'tenant-1',
          'slug': 'tenant-main',
        },
      },
    );

    final container = ProviderContainer(
      overrides: [
        authSessionProvider.overrideWith(
          (ref) async => const AuthSession(
            accessToken: 'token',
            refreshToken: 'refresh',
            expiresAt: null,
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
    expect(value.grantedPermissions, ['modules:manage']);
    expect(value.hasPermission('modules:manage'), isTrue);
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
            expiresAt: null,
          ),
        ),
        graphQlClientProvider.overrideWithValue(clientWithResult(queryResult)),
      ],
    );
    addTearDown(container.dispose);

    container.listen(authBootstrapProbeProvider, (_, __) {});
    await Future<void>.delayed(Duration.zero);

    expect(
      container.read(authBootstrapProbeProvider).error,
      isA<OperationException>(),
    );
  });

  test('throws when probe payload is missing required fields', () async {
    final queryResult = QueryResult<Object?>(
      options: QueryOptions(document: gql('query BootstrapProbe { me { id } }')),
      source: QueryResultSource.network,
      data: {
        '__typename': 'Query',
        'me': {
          '__typename': 'User',
          'id': '1',
          'email': null,
          'permissions': <String>[],
        },
        'currentTenant': {
          '__typename': 'Tenant',
          'id': 'tenant-1',
          'slug': null,
        },
      },
    );

    final container = ProviderContainer(
      overrides: [
        authSessionProvider.overrideWith(
          (ref) async => const AuthSession(
            accessToken: 'token',
            refreshToken: 'refresh',
            expiresAt: null,
          ),
        ),
        graphQlClientProvider.overrideWithValue(clientWithResult(queryResult)),
      ],
    );
    addTearDown(container.dispose);

    container.listen(authBootstrapProbeProvider, (_, __) {});
    await Future<void>.delayed(Duration.zero);

    expect(
      container.read(authBootstrapProbeProvider).error,
      isA<FormatException>(),
    );
  });
}
