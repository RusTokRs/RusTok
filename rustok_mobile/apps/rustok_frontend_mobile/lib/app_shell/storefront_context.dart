import 'dart:convert';
import 'dart:io';

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
const _defaultCartIdFile = String.fromEnvironment(
  'RUSTOK_STOREFRONT_CART_ID_FILE',
  defaultValue: '',
);
const _cartStorageKey = String.fromEnvironment(
  'RUSTOK_STOREFRONT_CART_STORAGE_KEY',
  defaultValue: 'rustok.storefront.cart_id',
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
    cartIdFilePath: _defaultCartIdFile.isEmpty ? null : _defaultCartIdFile,
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
  final filePath = _normalizeCartId(runtime.cartIdFilePath);
  final persistence = filePath == null
      ? InMemoryStorefrontCartIdPersistence(
          initialCartId: runtime.cartId,
          initialKey: _cartStorageKey,
        )
      : FileStorefrontCartIdPersistence(
          File(filePath),
          initialCartId: runtime.cartId,
        );

  return DurableStorefrontCartIdStore(
    persistence: persistence,
    key: _cartStorageKey,
  );
});

abstract interface class StorefrontCartIdStore {
  String? read();
  void write(String? cartId);
  void clear();
}

abstract interface class StorefrontCartIdPersistence {
  String? readCartId(String key);
  void writeCartId(String key, String cartId);
  void removeCartId(String key);
}

class DurableStorefrontCartIdStore implements StorefrontCartIdStore {
  DurableStorefrontCartIdStore({
    required StorefrontCartIdPersistence persistence,
    required String key,
  })  : _persistence = persistence,
        _key = key;

  final StorefrontCartIdPersistence _persistence;
  final String _key;

  @override
  String? read() => _normalizeCartId(_persistence.readCartId(_key));

  @override
  void write(String? cartId) {
    final normalized = _normalizeCartId(cartId);
    if (normalized == null) {
      clear();
      return;
    }
    _persistence.writeCartId(_key, normalized);
  }

  @override
  void clear() {
    _persistence.removeCartId(_key);
  }
}

class InMemoryStorefrontCartIdPersistence implements StorefrontCartIdPersistence {
  InMemoryStorefrontCartIdPersistence({
    String? initialCartId,
    String initialKey = _cartStorageKey,
  }) {
    final normalized = _normalizeCartId(initialCartId);
    if (normalized != null) {
      _values[initialKey] = normalized;
    }
  }

  final Map<String, String> _values = <String, String>{};

  @override
  String? readCartId(String key) => _values[key];

  @override
  void writeCartId(String key, String cartId) {
    _values[key] = cartId;
  }

  @override
  void removeCartId(String key) {
    _values.remove(key);
  }
}

class FileStorefrontCartIdPersistence implements StorefrontCartIdPersistence {
  FileStorefrontCartIdPersistence(
    this.file, {
    String? initialCartId,
  }) : _initialCartId = _normalizeCartId(initialCartId);

  final File file;
  final String? _initialCartId;

  @override
  String? readCartId(String key) {
    final values = _readValues();
    final persisted = _normalizeCartId(values[key]);
    if (persisted != null) {
      return persisted;
    }
    if (_initialCartId != null) {
      writeCartId(key, _initialCartId);
    }
    return _initialCartId;
  }

  @override
  void writeCartId(String key, String cartId) {
    final values = _readValues()..[key] = cartId;
    _writeValues(values);
  }

  @override
  void removeCartId(String key) {
    final values = _readValues()..remove(key);
    _writeValues(values);
  }

  Map<String, String> _readValues() {
    if (!file.existsSync()) {
      return <String, String>{};
    }

    try {
      final decoded = jsonDecode(file.readAsStringSync());
      if (decoded is Map<String, dynamic>) {
        return decoded.map(
          (key, value) => MapEntry(key, value is String ? value : '$value'),
        );
      }
    } on FormatException {
      return <String, String>{};
    } on FileSystemException {
      return <String, String>{};
    }

    return <String, String>{};
  }

  void _writeValues(Map<String, String> values) {
    file.parent.createSync(recursive: true);
    file.writeAsStringSync(jsonEncode(values));
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
    this.cartIdFilePath,
  });

  final String serverBaseUrl;
  final String tenantSlug;
  final String locale;
  final String? cartId;
  final String? cartIdFilePath;
}
