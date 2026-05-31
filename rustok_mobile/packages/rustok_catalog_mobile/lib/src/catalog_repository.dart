import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'product_summary.dart';

abstract interface class StorefrontCatalogRepository {
  Future<List<StorefrontProductSummary>> featuredProducts();

  Future<List<StorefrontCartLine>> cartLines();
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
