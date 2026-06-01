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
const _defaultCartId = String.fromEnvironment(
  'RUSTOK_STOREFRONT_CART_ID',
  defaultValue: '',
);

Uri _serverBaseUri(String serverBaseUrl) => Uri.parse(serverBaseUrl);

final storefrontRuntimeContextProvider = Provider<StorefrontRuntimeContext>((
  ref,
) {
  return StorefrontRuntimeContext(
    serverBaseUrl: _defaultServerBaseUrl,
    tenantSlug: _defaultTenantSlug,
    locale: _defaultLocale,
    cartId: _defaultCartId.isEmpty ? null : _defaultCartId,
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

final storefrontCartIdStoreProvider = Provider<StorefrontCartIdStore>((ref) {
  final runtime = ref.watch(storefrontRuntimeContextProvider);
  return InMemoryStorefrontCartIdStore(runtime.cartId);
});

abstract interface class StorefrontCartIdStore {
  String? read();
  void write(String? cartId);
  void clear();
}

class InMemoryStorefrontCartIdStore implements StorefrontCartIdStore {
  InMemoryStorefrontCartIdStore(String? initialCartId)
      : _cartId = _normalizeCartId(initialCartId);

  String? _cartId;

  @override
  String? read() => _cartId;

  @override
  void write(String? cartId) {
    _cartId = _normalizeCartId(cartId);
  }

  @override
  void clear() {
    _cartId = null;
  }
}

String? _normalizeCartId(String? cartId) {
  final trimmed = cartId?.trim();
  return trimmed == null || trimmed.isEmpty ? null : trimmed;
}

class StorefrontRuntimeContext {
  const StorefrontRuntimeContext({
    required this.serverBaseUrl,
    required this.tenantSlug,
    required this.locale,
    this.cartId,
  });

  final String serverBaseUrl;
  final String tenantSlug;
  final String locale;
  final String? cartId;
}
