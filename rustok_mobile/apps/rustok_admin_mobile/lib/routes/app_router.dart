import 'package:app_route_contracts/app_route_contracts.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:rustok_modules_mobile/rustok_modules_mobile.dart';

import '../app_shell/app_shell_page.dart';
import '../registry/mobile_module_icons.dart';
import '../registry/module_entry_adapter.dart';
import '../registry/registry_adaptation_summary.dart';
import 'registry_warnings_card.dart';

const _routeCodec = RouteCodec(
  RouteSanitizer({
    QueryKeys.tab,
    QueryKeys.productId,
    QueryKeys.orderId,
    QueryKeys.mediaId,
  }),
);

GoRouter buildRouter(
  ModuleRegistryAdaptationResult registryReport, {
  required ModulesRepository modulesRepository,
}) {
  final moduleRoutes = registryReport.routes;
  final summary = buildRegistryAdaptationSummary(registryReport);
  return GoRouter(
    initialLocation: modulesRootPath,
    routes: [
      ShellRoute(
        builder: (context, state, child) => AppShellPage(child: child),
        routes: [
          GoRoute(
            path: modulesRootPath,
            builder: (context, state) => ProviderScope(
              overrides: [
                modulesRepositoryProvider.overrideWithValue(modulesRepository),
              ],
              child: ModulesMobileScreen(
                header: ModulesHomePage(
                  moduleRoutes: moduleRoutes,
                  adaptationSummary: summary,
                  shrinkWrap: true,
                ),
                resolveModulePath: (module) => _resolveModulePath(
                  moduleRoutes,
                  module.slug,
                ),
                onOpenModule: (context, module) {
                  final path = _resolveModulePath(moduleRoutes, module.slug);
                  if (path != null) {
                    context.go(path);
                  }
                },
              ),
            ),
            routes: [
              for (final routeEntry in moduleRoutes)
                GoRoute(
                  path: routeEntry.routeSegment,
                  name: routeEntry.moduleKey,
                  builder: (context, state) {
                    final selection = _routeCodec.decode(
                      state.uri.path,
                      state.uri.queryParameters,
                    );
                    return ModulePlaceholderPage(
                      moduleRoute: routeEntry,
                      selection: selection,
                    );
                  },
                  routes: [
                    for (final child in routeEntry.childRoutes)
                      GoRoute(
                        path: child.subpath,
                        name: '${routeEntry.moduleKey}:${child.subpath}',
                        builder: (context, state) => ModuleChildPlaceholderPage(
                          moduleRoute: routeEntry,
                          childRoute: child,
                        ),
                      ),
                  ],
                ),
            ],
          ),
        ],
      ),
    ],
  );
}

String? _resolveModulePath(List<ModuleRouteEntry> routes, String moduleSlug) {
  final normalizedSlug = moduleSlug.trim().toLowerCase().replaceAll('-', '_');
  if (normalizedSlug.isEmpty) {
    return null;
  }

  final expectedModuleKey = 'rustok_$normalizedSlug';
  for (final route in routes) {
    if (route.moduleKey == expectedModuleKey ||
        route.routeSegment == normalizedSlug ||
        route.routeSegment.replaceAll('-', '_') == normalizedSlug) {
      return route.path;
    }
  }

  return null;
}

class ModulesHomePage extends StatelessWidget {
  const ModulesHomePage({
    super.key,
    required this.moduleRoutes,
    required this.adaptationSummary,
    this.shrinkWrap = false,
  });

  final List<ModuleRouteEntry> moduleRoutes;
  final RegistryAdaptationSummary adaptationSummary;
  final bool shrinkWrap;

  @override
  Widget build(BuildContext context) {
    final children = <Widget>[
      const ListTile(title: Text('RusTok Modules')),
      RegistryWarningsCard(summary: adaptationSummary),
      for (final moduleRoute in moduleRoutes)
        ExpansionTile(
          leading: Icon(iconForModuleRoute(moduleRoute)),
          title: Text(moduleRoute.navTitle),
          subtitle: Text(moduleRoute.path),
          children: [
            ListTile(
              title: const Text('Open module root'),
              onTap: () => context.go(moduleRoute.path),
            ),
            for (final child in moduleRoute.childRoutes)
              ListTile(
                title: Text(child.navLabel),
                subtitle: Text(child.path),
                onTap: () => context.go(child.path),
              ),
          ],
        ),
    ];

    if (shrinkWrap) {
      return Column(children: children);
    }

    return ListView(children: children);
  }
}

class ModulePlaceholderPage extends StatelessWidget {
  const ModulePlaceholderPage({
    super.key,
    required this.moduleRoute,
    required this.selection,
  });

  final ModuleRouteEntry moduleRoute;
  final RouteSelection selection;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(iconForModuleRoute(moduleRoute), size: 48),
          const SizedBox(height: 12),
          Text('Module: ${moduleRoute.moduleKey}'),
          Text('Icon: ${moduleRoute.navIcon}'),
          Text('Location: ${selection.toLocation()}'),
        ],
      ),
    );
  }
}

class ModuleChildPlaceholderPage extends StatelessWidget {
  const ModuleChildPlaceholderPage({
    super.key,
    required this.moduleRoute,
    required this.childRoute,
  });

  final ModuleRouteEntry moduleRoute;
  final ModuleChildRouteEntry childRoute;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(iconForModuleRoute(moduleRoute), size: 48),
          const SizedBox(height: 12),
          Text('Module: ${moduleRoute.moduleKey}'),
          Text('Child page: ${childRoute.title}'),
          Text('Path: ${childRoute.path}'),
        ],
      ),
    );
  }
}
