import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
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
    final router = buildStorefrontRouter();

    await tester.pumpWidget(
      ProviderScope(child: MaterialApp.router(routerConfig: router)),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Catalog').last);
    await tester.pumpAndSettle();
    expect(
      find.text('Product listing surface will mount here.'),
      findsOneWidget,
    );

    router.go('$storefrontModulesRootPath/blog');
    await tester.pumpAndSettle();
    expect(find.text('Module: blog'), findsOneWidget);
  });
}
