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
      id
      lineItems {
        id
        productId
        variantId
        title
        quantity
        totalPrice
        currencyCode
      }
    }
  }
''';

const storefrontMobileCreateCartMutation = r'''
  mutation StorefrontMobileCreateCart($input: CreateStorefrontCartInput!) {
    createStorefrontCart(input: $input) {
      cart {
        id
        lineItems {
          id
          productId
          variantId
          title
          quantity
          totalPrice
          currencyCode
        }
      }
    }
  }
''';

const storefrontMobileAddCartLineMutation = r'''
  mutation StorefrontMobileAddCartLine(
    $cartId: UUID!
    $input: AddStorefrontCartLineItemInput!
  ) {
    addStorefrontCartLineItem(cartId: $cartId, input: $input) {
      id
      lineItems {
        id
        productId
        variantId
        title
        quantity
        totalPrice
        currencyCode
      }
    }
  }
''';

const storefrontMobileUpdateCartLineMutation = r'''
  mutation StorefrontMobileUpdateCartLine(
    $cartId: UUID!
    $lineId: UUID!
    $input: UpdateStorefrontCartLineItemInput!
  ) {
    updateStorefrontCartLineItem(
      cartId: $cartId
      lineId: $lineId
      input: $input
    ) {
      id
      lineItems {
        id
        productId
        variantId
        title
        quantity
        totalPrice
        currencyCode
      }
    }
  }
''';

const storefrontMobileRemoveCartLineMutation = r'''
  mutation StorefrontMobileRemoveCartLine($cartId: UUID!, $lineId: UUID!) {
    removeStorefrontCartLineItem(cartId: $cartId, lineId: $lineId) {
      id
      lineItems {
        id
        productId
        variantId
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
  final cartIdStore = ref.watch(storefrontCartIdStoreProvider);
  return GraphQlStorefrontCatalogRepository(
    client: client,
    locale: runtime.locale,
    cartIdStore: cartIdStore,
  );
});

class GraphQlStorefrontCatalogRepository implements StorefrontCatalogRepository {
  GraphQlStorefrontCatalogRepository({
    required GraphQLClient client,
    required this.locale,
    required StorefrontCartIdStore cartIdStore,
  })  : _client = client,
        _cartIdStore = cartIdStore;

  final GraphQLClient _client;
  final String locale;
  final StorefrontCartIdStore _cartIdStore;

  String? get cartId => _cartIdStore.read();

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

    return _cartLinesFromPayload(payload);
  }

  @override
  Future<StorefrontCartWriteResult> createCart(
    StorefrontCreateCartDraft draft,
  ) async {
    final input = <String, dynamic>{
      if (_nonEmpty(draft.email) != null) 'email': _nonEmpty(draft.email),
      if (_nonEmpty(draft.currencyCode) != null)
        'currencyCode': _nonEmpty(draft.currencyCode),
      if (_nonEmpty(draft.countryCode) != null)
        'countryCode': _nonEmpty(draft.countryCode),
      'locale': _nonEmpty(draft.locale) ?? locale,
      'metadata': '{"source":"rustok-flutter-storefront"}',
    };
    final result = await _client.mutate(
      MutationOptions(
        document: gql(storefrontMobileCreateCartMutation),
        fetchPolicy: FetchPolicy.noCache,
        variables: <String, dynamic>{'input': input},
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = _readCartPayload(result.data, 'createStorefrontCart');
    return _rememberCart(payload);
  }

  @override
  Future<StorefrontCartWriteResult> addCartLine(
    StorefrontAddCartLineDraft draft,
  ) async {
    final id = await _ensureCartId();
    final result = await _client.mutate(
      MutationOptions(
        document: gql(storefrontMobileAddCartLineMutation),
        fetchPolicy: FetchPolicy.noCache,
        variables: <String, dynamic>{
          'cartId': id,
          'input': <String, dynamic>{
            'variantId': draft.variantId,
            'quantity': draft.quantity,
            'metadata': '{"source":"rustok-flutter-storefront"}',
          },
        },
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = _readCartPayload(result.data, 'addStorefrontCartLineItem');
    return _rememberCart(payload);
  }

  @override
  Future<StorefrontCartWriteResult> updateCartLine(
    StorefrontUpdateCartLineDraft draft,
  ) async {
    final id = _requireCartId();
    final result = await _client.mutate(
      MutationOptions(
        document: gql(storefrontMobileUpdateCartLineMutation),
        fetchPolicy: FetchPolicy.noCache,
        variables: <String, dynamic>{
          'cartId': id,
          'lineId': draft.lineId,
          'input': <String, dynamic>{'quantity': draft.quantity},
        },
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = _readCartPayload(
      result.data,
      'updateStorefrontCartLineItem',
    );
    return _rememberCart(payload);
  }

  @override
  Future<StorefrontCartWriteResult> removeCartLine(String lineId) async {
    final id = _requireCartId();
    final result = await _client.mutate(
      MutationOptions(
        document: gql(storefrontMobileRemoveCartLineMutation),
        fetchPolicy: FetchPolicy.noCache,
        variables: <String, dynamic>{'cartId': id, 'lineId': lineId},
      ),
    );

    if (result.hasException) {
      throw result.exception!;
    }

    final payload = _readCartPayload(
      result.data,
      'removeStorefrontCartLineItem',
    );
    return _rememberCart(payload);
  }

  StorefrontCartWriteResult _rememberCart(Map<String, dynamic> payload) {
    final id = _readString(payload, 'id');
    if (id.isNotEmpty) {
      _cartIdStore.write(id);
    }
    return StorefrontCartWriteResult(
      cartId: _cartIdStore.read() ?? id,
      lines: _cartLinesFromPayload(payload),
    );
  }

  Future<String> _ensureCartId() async {
    final id = cartId?.trim();
    if (id != null && id.isNotEmpty) {
      return id;
    }
    final created = await createCart(StorefrontCreateCartDraft(locale: locale));
    return created.cartId;
  }

  String _requireCartId() {
    final id = cartId?.trim();
    if (id == null || id.isEmpty) {
      throw StateError('Create a storefront cart before changing cart lines.');
    }
    return id;
  }
}

Map<String, dynamic> _readCartPayload(
  Map<String, dynamic>? data,
  String rootKey,
) {
  final payload = data?[rootKey];
  if (payload is! Map<String, dynamic>) {
    return const <String, dynamic>{};
  }
  final nestedCart = payload['cart'];
  if (nestedCart is Map<String, dynamic>) {
    return nestedCart;
  }
  return payload;
}

List<StorefrontCartLine> _cartLinesFromPayload(Map<String, dynamic> payload) {
  final items = payload['lineItems'];
  if (items is! List) {
    return const <StorefrontCartLine>[];
  }

  return List.unmodifiable(
    items.whereType<Map<String, dynamic>>().map(_cartLineFromJson),
  );
}

StorefrontCartLine _cartLineFromJson(Map<String, dynamic> item) {
  final lineId = _readString(item, 'id');
  final productId = _readOptionalString(item, 'productId') ??
      _readOptionalString(item, 'product_id') ??
      _readOptionalString(item, 'variantId') ??
      _readOptionalString(item, 'variant_id') ??
      _readString(item, 'title');
  final quantity = item['quantity'];
  return StorefrontCartLine(
    lineId: lineId,
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
    variantId: _readOptionalString(details, 'variantId') ??
        _readOptionalString(details, 'variant_id'),
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

String? _nonEmpty(String? value) {
  final trimmed = value?.trim();
  return trimmed == null || trimmed.isEmpty ? null : trimmed;
}

String _readString(Map<String, dynamic> json, String key) {
  final value = json[key];
  return value is String ? value : '';
}

String? _readOptionalString(Map<String, dynamic> json, String key) {
  final value = json[key];
  return value is String && value.isNotEmpty ? value : null;
}
