import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_catalog_mobile/rustok_catalog_mobile.dart';

void main() {
  testWidgets('catalog screen renders host-provided products', (tester) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          storefrontCatalogRepositoryProvider.overrideWithValue(
            const _FakeCatalogRepository(),
          ),
        ],
        child: const MaterialApp(home: StorefrontCatalogScreen()),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Catalog'), findsOneWidget);
    expect(find.text('Starter hoodie'), findsOneWidget);
    expect(find.text('Featured'), findsOneWidget);
    expect(find.text('24.00 USD'), findsOneWidget);
  });

  testWidgets('cart screen renders customer cart lines', (tester) async {
    var openedCatalog = false;

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          storefrontCatalogRepositoryProvider.overrideWithValue(
            const _FakeCatalogRepository(),
          ),
        ],
        child: MaterialApp(
          home: StorefrontCartScreen(
            onContinueShopping: () => openedCatalog = true,
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(
      find.text('Customer checkout preview without admin affordances.'),
      findsOneWidget,
    );
    expect(find.text('Starter hoodie'), findsOneWidget);
    expect(find.text('1×'), findsOneWidget);

    await tester.tap(find.text('Continue shopping'));
    expect(openedCatalog, isTrue);
  });

  testWidgets('cart screen renders empty action', (tester) async {
    var openedCatalog = false;

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          storefrontCatalogRepositoryProvider.overrideWithValue(
            const _EmptyCatalogRepository(),
          ),
        ],
        child: MaterialApp(
          home: StorefrontCartScreen(
            onContinueShopping: () => openedCatalog = true,
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Cart is empty'), findsOneWidget);
    await tester.tap(find.text('Open catalog'));
    expect(openedCatalog, isTrue);
  });
}

class _FakeCatalogRepository implements StorefrontCatalogRepository {
  const _FakeCatalogRepository();

  @override
  Future<List<StorefrontProductSummary>> featuredProducts() async {
    return const [
      StorefrontProductSummary(
        id: 'starter-hoodie',
        title: 'Starter hoodie',
        description: 'A storefront product card owned by the catalog package.',
        priceLabel: '24.00 USD',
        variantId: 'starter-hoodie-variant',
        badge: 'Featured',
      ),
    ];
  }

  @override
  Future<List<StorefrontCartLine>> cartLines() async {
    return const [
      StorefrontCartLine(
        lineId: 'line-starter-hoodie',
        productId: 'starter-hoodie',
        title: 'Starter hoodie',
        quantity: 1,
        priceLabel: '24.00 USD',
      ),
    ];
  }

  @override
  Future<StorefrontCartWriteResult> createCart(
    StorefrontCreateCartDraft draft,
  ) async {
    return const StorefrontCartWriteResult(
      cartId: 'cart-1',
      lines: <StorefrontCartLine>[],
    );
  }

  @override
  Future<StorefrontCartWriteResult> addCartLine(
    StorefrontAddCartLineDraft draft,
  ) async {
    return StorefrontCartWriteResult(
      cartId: 'cart-1',
      lines: await cartLines(),
    );
  }

  @override
  Future<StorefrontCartWriteResult> updateCartLine(
    StorefrontUpdateCartLineDraft draft,
  ) async {
    return StorefrontCartWriteResult(
      cartId: 'cart-1',
      lines: await cartLines(),
    );
  }

  @override
  Future<StorefrontCartWriteResult> removeCartLine(String lineId) async {
    return const StorefrontCartWriteResult(
      cartId: 'cart-1',
      lines: <StorefrontCartLine>[],
    );
  }
}

class _EmptyCatalogRepository implements StorefrontCatalogRepository {
  const _EmptyCatalogRepository();

  @override
  Future<List<StorefrontProductSummary>> featuredProducts() async {
    return const <StorefrontProductSummary>[];
  }

  @override
  Future<List<StorefrontCartLine>> cartLines() async {
    return const <StorefrontCartLine>[];
  }

  @override
  Future<StorefrontCartWriteResult> createCart(
    StorefrontCreateCartDraft draft,
  ) async {
    return const StorefrontCartWriteResult(
      cartId: 'cart-1',
      lines: <StorefrontCartLine>[],
    );
  }

  @override
  Future<StorefrontCartWriteResult> addCartLine(
    StorefrontAddCartLineDraft draft,
  ) async {
    return const StorefrontCartWriteResult(
      cartId: 'cart-1',
      lines: <StorefrontCartLine>[],
    );
  }

  @override
  Future<StorefrontCartWriteResult> updateCartLine(
    StorefrontUpdateCartLineDraft draft,
  ) async {
    return const StorefrontCartWriteResult(
      cartId: 'cart-1',
      lines: <StorefrontCartLine>[],
    );
  }

  @override
  Future<StorefrontCartWriteResult> removeCartLine(String lineId) async {
    return const StorefrontCartWriteResult(
      cartId: 'cart-1',
      lines: <StorefrontCartLine>[],
    );
  }
}
