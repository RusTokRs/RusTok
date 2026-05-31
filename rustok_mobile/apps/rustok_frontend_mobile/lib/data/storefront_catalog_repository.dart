import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:graphql/client.dart';
import 'package:rustok_catalog_mobile/rustok_catalog_mobile.dart';

import '../app_shell/storefront_context.dart';

const storefrontMobileCatalogQuery = r'''
  query StorefrontMobileCatalog($input: SearchPreviewInput!) {
    storefrontSearch(input: $input) {
      items {
        id
        entityType
        title
        snippet
        url
        payload
      }
    }
  }
''';

const storefrontMobileCartQuery = r'''
  query StorefrontMobileCart($id: UUID!) {
    storefrontCart(id: $id) {
      lineItems {
        productId
        title
        quantity
        totalPrice
        currencyCode
      }
    }
  }
''';

final hostStorefrontCatalogRepositoryProvider =
    Provider<StorefrontCatalogRepository>((ref) {
  final client = ref.watch(storefrontGraphQlClientProvider);
  final runtime = ref.watch(storefrontRuntimeContextProvider);
  return GraphQlStorefrontCatalogRepository(
    client: client,
    locale: runtime.locale,
    cartId: runtime.cartId,
  );
});

class GraphQlStorefrontCatalogRepository implements StorefrontCatalogRepository {
  const GraphQlStorefrontCatalogRepository({
    required GraphQLClient client,
    required this.locale,
    this.cartId,
  }) : _client = client;

  final GraphQLClient _client;
  final String locale;
  final String? cartId;

  @override
  Future<List<StorefrontProductSummary>> featuredProducts() async {
    final result = await _client.query(
      QueryOptions(
        document: gql(storefrontMobileCatalogQuery),
        fetchPolicy: FetchPolicy.cacheAndNetwork,
        variables: <String, dynamic>{
          'input': <String, dynamic>{
            'query': '',
            'locale': locale,
            'limit': 12,
            'entityTypes': <String>['product'],
          },
        },
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = result.data?['storefrontSearch'];
    if (payload is! Map<String, dynamic>) {
      return const <StorefrontProductSummary>[];
    }

    final items = payload['items'];
    if (items is! List) {
      return const <StorefrontProductSummary>[];
    }

    return List.unmodifiable(
      items
          .whereType<Map<String, dynamic>>()
          .where((item) => item['entityType'] == 'product')
          .map(_productFromSearchItem),
    );
  }

  @override
  Future<List<StorefrontCartLine>> cartLines() async {
    final id = cartId?.trim();
    if (id == null || id.isEmpty) {
      return const <StorefrontCartLine>[];
    }

    final result = await _client.query(
      QueryOptions(
        document: gql(storefrontMobileCartQuery),
        fetchPolicy: FetchPolicy.cacheAndNetwork,
        variables: <String, dynamic>{'id': id},
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = result.data?['storefrontCart'];
    if (payload is! Map<String, dynamic>) {
      return const <StorefrontCartLine>[];
    }

    final items = payload['lineItems'];
    if (items is! List) {
      return const <StorefrontCartLine>[];
    }

    return List.unmodifiable(
      items.whereType<Map<String, dynamic>>().map(_cartLineFromJson),
    );
  }
}

StorefrontCartLine _cartLineFromJson(Map<String, dynamic> item) {
  final productId = _readOptionalString(item, 'productId') ??
      _readOptionalString(item, 'variantId') ??
      _readString(item, 'title');
  final quantity = item['quantity'];
  return StorefrontCartLine(
    productId: productId,
    title: _readString(item, 'title'),
    quantity: quantity is int ? quantity : 0,
    priceLabel: _cartLinePriceLabel(item),
  );
}

String _cartLinePriceLabel(Map<String, dynamic> item) {
  final total = _readOptionalString(item, 'totalPrice') ??
      _readOptionalString(item, 'total_price');
  final currency = _readOptionalString(item, 'currencyCode') ??
      _readOptionalString(item, 'currency_code');
  if (total == null) {
    return currency ?? '';
  }
  if (currency == null) {
    return total;
  }
  return '$total $currency';
}

StorefrontProductSummary _productFromSearchItem(Map<String, dynamic> item) {
  final details = _decodePayload(item['payload']);
  final id = _readString(item, 'id');
  final title = _readString(item, 'title');
  final snippet = _readOptionalString(item, 'snippet');
  final url = _readOptionalString(item, 'url');

  return StorefrontProductSummary(
    id: id,
    title: title.isNotEmpty ? title : id,
    description: snippet ?? url ?? 'Published storefront product',
    priceLabel: _priceLabel(details),
    badge: _readOptionalString(details, 'badge'),
  );
}

Map<String, dynamic> _decodePayload(Object? value) {
  if (value is! String || value.trim().isEmpty) {
    return const <String, dynamic>{};
  }

  try {
    final decoded = jsonDecode(value);
    if (decoded is Map<String, dynamic>) {
      return decoded;
    }
  } on FormatException {
    return const <String, dynamic>{};
  }
  return const <String, dynamic>{};
}

String _priceLabel(Map<String, dynamic> payload) {
  final price = _readOptionalString(payload, 'price') ??
      _readOptionalString(payload, 'priceLabel') ??
      _readOptionalString(payload, 'price_label');
  return price ?? 'Open product details';
}

String _readString(Map<String, dynamic> json, String key) {
  final value = json[key];
  return value is String ? value : '';
}

String? _readOptionalString(Map<String, dynamic> json, String key) {
  final value = json[key];
  return value is String && value.isNotEmpty ? value : null;
}
