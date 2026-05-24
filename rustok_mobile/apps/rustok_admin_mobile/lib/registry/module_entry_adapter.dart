import 'package:app_module_contracts/app_module_contracts.dart';

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

List<ModuleRouteEntry> adaptModuleEntries(List<MobileModuleEntry> entries) {
  return List.unmodifiable(
    entries.map((entry) {
      final routeSegment = _sanitizeSegment(entry.routeSegment);
      final basePath = '/modules/$routeSegment';
      final childRoutes = entry.childPages.map((child) {
        final subpath = _sanitizeSegment(child.subpath);
        return ModuleChildRouteEntry(
          subpath: subpath,
          path: '$basePath/$subpath',
          title: child.title,
          navLabel: child.navLabel ?? child.title,
        );
      }).toList(growable: false);

      return ModuleRouteEntry(
        moduleKey: entry.moduleKey,
        routeSegment: routeSegment,
        path: basePath,
        navTitle: entry.nav.title,
        childRoutes: childRoutes,
      );
    }),
  );
}

String _sanitizeSegment(String value) {
  return value.trim().replaceAll(RegExp(r'^/+|/+$'), '');
}
