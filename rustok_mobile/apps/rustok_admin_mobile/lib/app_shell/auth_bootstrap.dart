import 'package:app_graphql/app_graphql.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:graphql/client.dart';

const _defaultServerBaseUrl = String.fromEnvironment(
  'RUSTOK_SERVER_BASE_URL',
  defaultValue: 'http://localhost:8080',
);
const _defaultTenantSlug = String.fromEnvironment(
  'RUSTOK_TENANT_SLUG',
  defaultValue: 'default',
);
const _defaultLocale = String.fromEnvironment(
  'RUSTOK_LOCALE',
  defaultValue: 'en',
);

Uri _serverBaseUri() => Uri.parse(_defaultServerBaseUrl);

final authSessionStoreProvider = Provider<AuthSessionStore>((ref) {
  return InMemoryAuthSessionStore();
});

final refreshClientProvider = Provider<GraphQLClient>((ref) {
  final config = GraphQlClientConfig(
    baseUri: _serverBaseUri(),
    context: const GraphQlRequestContext(
      tenantSlug: _defaultTenantSlug,
      locale: _defaultLocale,
    ),
  );
  return const GraphQlClientFactory().createHttpOnly(config);
});

final refreshTokenServiceProvider = Provider<RefreshTokenService>((ref) {
  final client = ref.watch(refreshClientProvider);
  return GraphQlRefreshTokenService(client: client);
});

final authSessionManagerProvider = Provider<AuthSessionManager>((ref) {
  final store = ref.watch(authSessionStoreProvider);
  final refreshService = ref.watch(refreshTokenServiceProvider);
  return AuthSessionManager(store: store, refreshTokenService: refreshService);
});

final authSessionProvider = FutureProvider<AuthSession?>((ref) async {
  final manager = ref.watch(authSessionManagerProvider);
  return manager.readValidSession();
});

final graphQlConfigProvider = Provider<GraphQlClientConfig>((ref) {
  final session = ref.watch(authSessionProvider).valueOrNull;
  return GraphQlClientConfig(
    baseUri: _serverBaseUri(),
    context: GraphQlRequestContext(
      tenantSlug: _defaultTenantSlug,
      locale: _defaultLocale,
      accessToken: session?.accessToken,
    ),
  );
});

final graphQlClientProvider = Provider<GraphQLClient>((ref) {
  final config = ref.watch(graphQlConfigProvider);
  return const GraphQlClientFactory().create(config);
});

const _bootstrapProbeDocument = r'''
  query BootstrapProbe {
    me {
      id
      email
    }
    currentTenant {
      id
      slug
    }
  }
''';

final authBootstrapProbeProvider = FutureProvider<BootstrapProbeResult>((ref) async {
  final session = await ref.watch(authSessionProvider.future);
  if (session == null) {
    return const BootstrapProbeResult.unauthenticated();
  }

  final client = ref.watch(graphQlClientProvider);
  final result = await client.query(
    QueryOptions(
      document: gql(_bootstrapProbeDocument),
      fetchPolicy: FetchPolicy.networkOnly,
    ),
  );
  if (result.hasException) {
    throw result.exception!;
  }

  final payload = result.data ?? const <String, dynamic>{};
  final userEmail = _readStringField(
    payload,
    objectField: 'me',
    scalarField: 'email',
  );
  final tenantSlug = _readStringField(
    payload,
    objectField: 'currentTenant',
    scalarField: 'slug',
  );
  if (userEmail == null || tenantSlug == null) {
    throw const FormatException(
      'Bootstrap probe payload is missing required me/currentTenant fields.',
    );
  }

  return BootstrapProbeResult.authenticated(
    userEmail: userEmail,
    tenantSlug: tenantSlug,
  );
});

String? _readStringField(
  Map<String, dynamic> payload, {
  required String objectField,
  required String scalarField,
}) {
  final nested = payload[objectField];
  if (nested is! Map<String, dynamic>) {
    return null;
  }
  final scalar = nested[scalarField];
  return scalar is String ? scalar : null;
}

class BootstrapProbeResult {
  const BootstrapProbeResult._({
    required this.isAuthenticated,
    this.userEmail,
    this.tenantSlug,
  });

  const BootstrapProbeResult.unauthenticated()
    : this._(isAuthenticated: false);

  const BootstrapProbeResult.authenticated({
    String? userEmail,
    String? tenantSlug,
  }) : this._(
         isAuthenticated: true,
         userEmail: userEmail,
         tenantSlug: tenantSlug,
       );

  final bool isAuthenticated;
  final String? userEmail;
  final String? tenantSlug;
}
