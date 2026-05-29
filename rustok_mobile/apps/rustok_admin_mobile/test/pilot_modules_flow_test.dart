import 'package:app_module_contracts/app_module_contracts.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_admin_mobile/app_shell/auth_bootstrap.dart';
import 'package:rustok_admin_mobile/registry/module_entry_adapter.dart';
import 'package:rustok_admin_mobile/routes/app_router.dart';
import 'package:rustok_modules_mobile/rustok_modules_mobile.dart';

void main() {
  testWidgets('pilot modules flow opens module detail and returns to shell', (
    tester,
  ) async {
    final router = buildRouter(
      const ModuleRegistryAdaptationResult(
        routes: [
          ModuleRouteEntry(
            moduleKey: 'rustok_blog',
            surfaceKind: MobileSurfaceKind.admin,
            routeSegment: 'blog',
            localeNamespace: 'blog',
            permissions: [],
            path: '/modules/blog',
            navTitle: 'Blog',
            navIcon: 'article',
            childRoutes: [],
          ),
        ],
        rejectedModuleEntries: 0,
        rejectedChildEntries: 0,
      ),
      modulesRepository: const _FakeModulesRepository(),
    );

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          authBootstrapProbeProvider.overrideWith(
            (ref) async => const BootstrapProbeResult.authenticated(
              userEmail: 'operator@example.com',
              tenantSlug: 'default',
            ),
          ),
        ],
        child: MaterialApp.router(routerConfig: router),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Modules pilot'), findsOneWidget);
    expect(find.text('Blog Module'), findsOneWidget);

    await tester.tap(find.text('Blog Module'));
    await tester.pumpAndSettle();

    expect(find.text('Module: rustok_blog'), findsOneWidget);
    expect(find.text('Icon: article'), findsOneWidget);

    router.go(modulesRootPath);
    await tester.pumpAndSettle();

    expect(find.text('Modules pilot'), findsOneWidget);
    expect(find.text('Blog Module'), findsOneWidget);
  });
}

class _FakeModulesRepository implements ModulesRepository {
  const _FakeModulesRepository();

  @override
  Future<List<ModuleSummary>> listModules() async => const [
    ModuleSummary(
      slug: 'blog',
      name: 'Blog Module',
      description: 'Editorial content',
      version: '1.0.0',
      kind: 'optional',
      enabled: true,
      ownership: 'platform',
      trustLevel: 'trusted',
      recommendedAdminSurfaces: ['posts'],
      showcaseAdminSurfaces: [],
    ),
  ];
}
