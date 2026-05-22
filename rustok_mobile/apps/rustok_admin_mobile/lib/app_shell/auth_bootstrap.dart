import 'package:app_graphql/app_graphql.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:graphql/client.dart';

const defaultServerBaseUrl = String.fromEnvironment(
  'RUSTOK_SERVER_BASE_URL',
  defaultValue: 'http://localhost:8080',
);
const defaultTenantSlug = String.fromEnvironment(
  'RUSTOK_TENANT_SLUG',
  defaultValue: 'default',
);
const defaultLocale = String.fromEnvironment(
  'RUSTOK_LOCALE',
  defaultValue: 'en',
);

final authSessionStoreProvider = Provider<AuthSessionStore>((ref) {
  return InMemoryAuthSessionStore();
});

final refreshClientProvider = Provider<GraphQLClient>((ref) {
  final config = GraphQlClientConfig(
    baseUri: Uri.parse(defaultServerBaseUrl),
    context: const GraphQlRequestContext(
      tenantSlug: defaultTenantSlug,
      locale: defaultLocale,
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
    baseUri: Uri.parse(defaultServerBaseUrl),
    context: GraphQlRequestContext(
      tenantSlug: defaultTenantSlug,
      locale: defaultLocale,
      accessToken: session?.accessToken,
    ),
  );
});

final graphQlClientProvider = Provider<GraphQLClient>((ref) {
  final config = ref.watch(graphQlConfigProvider);
  return const GraphQlClientFactory().create(config);
});
