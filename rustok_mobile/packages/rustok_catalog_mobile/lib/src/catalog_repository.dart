import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'product_summary.dart';

abstract interface class StorefrontCatalogRepository {
  Future<List<StorefrontProductSummary>> featuredProducts();

  Future<List<StorefrontCartLine>> cartLines();

  Future<StorefrontCartWriteResult> createCart(StorefrontCreateCartDraft draft);

  Future<StorefrontCartWriteResult> addCartLine(StorefrontAddCartLineDraft draft);

  Future<StorefrontCartWriteResult> updateCartLine(
    StorefrontUpdateCartLineDraft draft,
  );

  Future<StorefrontCartWriteResult> removeCartLine(String lineId);
}

final storefrontCatalogRepositoryProvider =
    Provider<StorefrontCatalogRepository>((ref) {
  throw UnimplementedError(
    'Host app must override storefrontCatalogRepositoryProvider with a host-owned repository.',
  );
});

final featuredProductsProvider =
    FutureProvider<List<StorefrontProductSummary>>((ref) {
  return ref.watch(storefrontCatalogRepositoryProvider).featuredProducts();
});

final cartLinesProvider = FutureProvider<List<StorefrontCartLine>>((ref) {
  return ref.watch(storefrontCatalogRepositoryProvider).cartLines();
});

class StorefrontCreateCartDraft {
  const StorefrontCreateCartDraft({
    this.email,
    this.currencyCode,
    this.countryCode,
    this.locale,
  });

  final String? email;
  final String? currencyCode;
  final String? countryCode;
  final String? locale;
}

class StorefrontAddCartLineDraft {
  const StorefrontAddCartLineDraft({
    required this.variantId,
    this.quantity = 1,
  });

  final String variantId;
  final int quantity;
}

class StorefrontUpdateCartLineDraft {
  const StorefrontUpdateCartLineDraft({
    required this.lineId,
    required this.quantity,
  });

  final String lineId;
  final int quantity;
}

class StorefrontCartWriteResult {
  const StorefrontCartWriteResult({
    required this.cartId,
    required this.lines,
  });

  final String cartId;
  final List<StorefrontCartLine> lines;
}
