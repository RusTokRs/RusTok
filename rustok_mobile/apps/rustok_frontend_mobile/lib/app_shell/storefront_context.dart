import 'package:app_graphql/app_graphql.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:graphql/client.dart';

const _defaultServerBaseUrl = String.fromEnvironment(
  'RUSTOK_STOREFRONT_SERVER_BASE_URL',
  defaultValue: 'http://localhost:8080',
);
const _defaultTenantSlug = String.fromEnvironment(
  'RUSTOK_STOREFRONT_TENANT_SLUG',
  defaultValue: 'default',
);
const _defaultLocale = String.fromEnvironment(
  'RUSTOK_STOREFRONT_LOCALE',
  defaultValue: 'en',
);

Uri _serverBaseUri(String serverBaseUrl) => Uri.parse(serverBaseUrl);

final storefrontRuntimeContextProvider = Provider<StorefrontRuntimeContext>((
  ref,
) {
  return const StorefrontRuntimeContext(
    serverBaseUrl: _defaultServerBaseUrl,
    tenantSlug: _defaultTenantSlug,
    locale: _defaultLocale,
  );
});

final storefrontGraphQlConfigProvider = Provider<GraphQlClientConfig>((ref) {
  final runtime = ref.watch(storefrontRuntimeContextProvider);
  return GraphQlClientConfig(
    baseUri: _serverBaseUri(runtime.serverBaseUrl),
    context: GraphQlRequestContext(
      tenantSlug: runtime.tenantSlug,
      locale: runtime.locale,
    ),
  );
});

final storefrontGraphQlClientProvider = Provider<GraphQLClient>((ref) {
  final config = ref.watch(storefrontGraphQlConfigProvider);
  return const GraphQlClientFactory().create(config);
});

class StorefrontRuntimeContext {
  const StorefrontRuntimeContext({
    required this.serverBaseUrl,
    required this.tenantSlug,
    required this.locale,
  });

  final String serverBaseUrl;
  final String tenantSlug;
  final String locale;
}
