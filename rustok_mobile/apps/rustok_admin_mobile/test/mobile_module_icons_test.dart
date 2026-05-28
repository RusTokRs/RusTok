import 'package:app_module_contracts/app_module_contracts.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_admin_mobile/registry/mobile_module_icons.dart';
import 'package:rustok_admin_mobile/registry/module_entry_adapter.dart';

void main() {
  ModuleRouteEntry route({
    required String moduleKey,
    required String routeSegment,
    required String navIcon,
  }) {
    return ModuleRouteEntry(
      moduleKey: moduleKey,
      surfaceKind: MobileSurfaceKind.admin,
      routeSegment: routeSegment,
      localeNamespace: null,
      permissions: const <String>[],
      path: '/modules/$routeSegment',
      navTitle: moduleKey,
      navIcon: navIcon,
      childRoutes: const <ModuleChildRouteEntry>[],
    );
  }

  test('uses explicit manifest icon when it is known and non-generic', () {
    expect(
      iconForModuleRoute(
        route(
          moduleKey: 'rustok_blog',
          routeSegment: 'blog',
          navIcon: 'article',
        ),
      ),
      Icons.article_outlined,
    );
  });

  test('maps generic module icon by module metadata fallback', () {
    expect(
      iconForModuleRoute(
        route(
          moduleKey: 'rustok_pages',
          routeSegment: 'pages',
          navIcon: 'module',
        ),
      ),
      Icons.web_stories_outlined,
    );
  });

  test('falls back to extension icon for unknown metadata', () {
    expect(
      iconForModuleRoute(
        route(
          moduleKey: 'rustok_unknown',
          routeSegment: 'unknown',
          navIcon: 'missing_icon',
        ),
      ),
      Icons.extension_outlined,
    );
  });
}
