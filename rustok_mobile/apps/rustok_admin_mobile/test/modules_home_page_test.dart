import 'package:app_module_contracts/app_module_contracts.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_admin_mobile/registry/module_entry_adapter.dart';
import 'package:rustok_admin_mobile/registry/registry_adaptation_summary.dart';
import 'package:rustok_admin_mobile/routes/app_router.dart';

void main() {
  Widget _wrap(Widget child) {
    return MaterialApp(home: Scaffold(body: child));
  }

  testWidgets('shows adaptation warning card when rejected counters are non-zero', (
    tester,
  ) async {
    await tester.pumpWidget(
      _wrap(
        const ModulesHomePage(
          moduleRoutes: <ModuleRouteEntry>[],
          adaptationSummary: RegistryAdaptationSummary(
            hasWarnings: true,
            message: 'Rejected modules: 2 · Rejected child pages: 1',
          ),
        ),
      ),
    );

    expect(find.text('Registry adaptation warnings'), findsOneWidget);
    expect(
      find.text('Rejected modules: 2 · Rejected child pages: 1'),
      findsOneWidget,
    );
  });

  testWidgets('does not show adaptation warning card when counters are zero', (
    tester,
  ) async {
    await tester.pumpWidget(
      _wrap(
        const ModulesHomePage(
          moduleRoutes: <ModuleRouteEntry>[],
          adaptationSummary: RegistryAdaptationSummary(
            hasWarnings: false,
            message: 'Registry adaptation completed with no rejected entries.',
          ),
        ),
      ),
    );

    expect(find.text('Registry adaptation warnings'), findsNothing);
  });

  testWidgets('renders module and child entries from adapted routes', (tester) async {
    const routes = <ModuleRouteEntry>[
      ModuleRouteEntry(
        moduleKey: 'rustok_blog',
        surfaceKind: MobileSurfaceKind.admin,
        routeSegment: 'blog',
        localeNamespace: null,
        permissions: <String>[],
        path: '/modules/blog',
        navTitle: 'Blog',
        navIcon: 'article',
        childRoutes: <ModuleChildRouteEntry>[
          ModuleChildRouteEntry(
            subpath: 'posts',
            path: '/modules/blog/posts',
            title: 'Posts',
            navLabel: 'All Posts',
          ),
        ],
      ),
    ];

    await tester.pumpWidget(
      _wrap(
        const ModulesHomePage(
          moduleRoutes: routes,
          adaptationSummary: RegistryAdaptationSummary(
            hasWarnings: false,
            message: 'Registry adaptation completed with no rejected entries.',
          ),
        ),
      ),
    );

    expect(find.text('Blog'), findsOneWidget);
    expect(find.text('/modules/blog'), findsOneWidget);

    await tester.tap(find.text('Blog'));
    await tester.pumpAndSettle();

    expect(find.text('All Posts'), findsOneWidget);
    expect(find.text('/modules/blog/posts'), findsOneWidget);
  });
}
