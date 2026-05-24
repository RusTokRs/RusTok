import 'package:app_module_contracts/app_module_contracts.dart';

const modulesRootPath = '/modules';

class ModuleRouteEntry {
  const ModuleRouteEntry({
    required this.moduleKey,
    required this.routeSegment,
    required this.path,
    required this.navTitle,
    required this.childRoutes,
  });

  final String moduleKey;
  final String routeSegment;
  final String path;
  final String navTitle;
  final List<ModuleChildRouteEntry> childRoutes;
}

class ModuleChildRouteEntry {
  const ModuleChildRouteEntry({
    required this.subpath,
    required this.path,
    required this.title,
    required this.navLabel,
  });

  final String subpath;
  final String path;
  final String title;
  final String navLabel;
}

class ModuleRegistryAdaptationResult {
  const ModuleRegistryAdaptationResult({
    required this.routes,
    required this.rejectedModuleEntries,
    required this.rejectedChildEntries,
  });

  final List<ModuleRouteEntry> routes;
  final int rejectedModuleEntries;
  final int rejectedChildEntries;
}

List<ModuleRouteEntry> adaptModuleEntries(List<MobileModuleEntry> entries) {
  return adaptModuleEntriesWithReport(entries).routes;
}

ModuleRegistryAdaptationResult adaptModuleEntriesWithReport(
  List<MobileModuleEntry> entries,
) {
  final adapted = <ModuleRouteEntry>[];
  final usedModuleKeys = <String>{};
  final usedRouteSegments = <String>{};

  var rejectedModules = 0;
  var rejectedChildren = 0;

  for (final entry in entries) {
    final moduleKey = entry.moduleKey.trim();
    final routeSegment = _sanitizeSegment(entry.routeSegment);
    final hasValidModuleIdentity = moduleKey.isNotEmpty && routeSegment.isNotEmpty;
    final hasUniqueIdentity =
        usedModuleKeys.add(moduleKey) && usedRouteSegments.add(routeSegment);

    if (!hasValidModuleIdentity || !hasUniqueIdentity) {
      rejectedModules += 1;
      continue;
    }

    final basePath = '$modulesRootPath/$routeSegment';
    final childRoutes = <ModuleChildRouteEntry>[];
    final usedChildSubpaths = <String>{};

    for (final child in entry.childPages) {
      final subpath = _sanitizeSegment(child.subpath);
      if (subpath.isEmpty || !usedChildSubpaths.add(subpath)) {
        rejectedChildren += 1;
        continue;
      }

      childRoutes.add(
        ModuleChildRouteEntry(
          subpath: subpath,
          path: '$basePath/$subpath',
          title: child.title,
          navLabel: child.navLabel ?? child.title,
        ),
      );
    }

    adapted.add(
      ModuleRouteEntry(
        moduleKey: moduleKey,
        routeSegment: routeSegment,
        path: basePath,
        navTitle: entry.nav.title,
        childRoutes: List.unmodifiable(childRoutes),
      ),
    );
  }

  return ModuleRegistryAdaptationResult(
    routes: List.unmodifiable(adapted),
    rejectedModuleEntries: rejectedModules,
    rejectedChildEntries: rejectedChildren,
  );
}

String _sanitizeSegment(String value) {
  final trimmed = value.trim().replaceAll(RegExp(r'^/+|/+$'), '').toLowerCase();
  if (trimmed.isEmpty) {
    return '';
  }

  const allowed = r'^[a-z0-9_-]+$';
  if (!RegExp(allowed).hasMatch(trimmed)) {
    return '';
  }

  return trimmed;
}
