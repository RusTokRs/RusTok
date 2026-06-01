import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_catalog_mobile/rustok_catalog_mobile.dart';
import 'package:rustok_frontend_mobile/app_shell/storefront_context.dart';
import 'package:rustok_frontend_mobile/routes/storefront_router.dart';

void main() {
  testWidgets('renders storefront home with host-owned runtime context', (
    tester,
  ) async {
    final router = buildStorefrontRouter();

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          storefrontRuntimeContextProvider.overrideWithValue(
            const StorefrontRuntimeContext(
              serverBaseUrl: 'https://example.test',
              tenantSlug: 'acme',
              locale: 'ru',
            ),
          ),
        ],
        child: MaterialApp.router(routerConfig: router),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('RusTok Storefront'), findsOneWidget);
    expect(find.text('Mobile storefront host'), findsOneWidget);
    expect(find.textContaining('tenant: acme · locale: ru'), findsOneWidget);
  });

  testWidgets('navigates to catalog and module placeholder routes', (
    tester,
  ) async {
    final router = buildStorefrontRouter(
      catalogRepository: const _FakeStorefrontCatalogRepository(),
    );

    await tester.pumpWidget(
      ProviderScope(child: MaterialApp.router(routerConfig: router)),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Catalog').last);
    await tester.pumpAndSettle();
    expect(
      find.text('Module-owned mobile surface mounted by the storefront host.'),
      findsOneWidget,
    );
    expect(find.text('Creator kit'), findsOneWidget);

    await tester.tap(find.text('Cart').last);
    await tester.pumpAndSettle();
    expect(
      find.text('Customer checkout preview without admin affordances.'),
      findsOneWidget,
    );

    router.go('$storefrontModulesRootPath/blog');
    await tester.pumpAndSettle();
    expect(find.text('Blog'), findsOneWidget);
    expect(
      find.text('Manifest-driven storefront mobile surface.'),
      findsOneWidget,
    );

    router.go('$storefrontModulesRootPath/products');
    await tester.pumpAndSettle();
    expect(find.text('Creator kit'), findsOneWidget);
    expect(
      find.text('Module-owned mobile surface mounted by the storefront host.'),
      findsOneWidget,
    );

    router.go('$storefrontModulesRootPath/cart');
    await tester.pumpAndSettle();
    expect(
      find.text('Customer checkout preview without admin affordances.'),
      findsOneWidget,
    );
  });
}

class _FakeStorefrontCatalogRepository implements StorefrontCatalogRepository {
  const _FakeStorefrontCatalogRepository();

  @override
  Future<List<StorefrontProductSummary>> featuredProducts() async {
    return const [
      StorefrontProductSummary(
        id: 'creator-kit',
        title: 'Creator kit',
        description: 'Mounted through the storefront shell.',
        priceLabel: '49.00 USD',
        variantId: 'creator-kit-variant',
      ),
    ];
  }

  @override
  Future<List<StorefrontCartLine>> cartLines() async {
    return const [
      StorefrontCartLine(
        lineId: 'line-starter-hoodie',
        productId: 'creator-kit',
        title: 'Creator kit',
        quantity: 1,
        priceLabel: '49.00 USD',
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
