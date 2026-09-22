import 'mobile_child_page.dart';
import 'mobile_nav_meta.dart';
import 'mobile_surface_kind.dart';

class MobileModuleEntry {
  const MobileModuleEntry({
    required this.moduleKey,
    this.surfaceKind = MobileSurfaceKind.admin,
    required this.routeSegment,
    this.localeNamespace,
    this.permissions = const <String>[],
    required this.nav,
    this.childPages = const <MobileChildPage>[],
  });

  final String moduleKey;
  final MobileSurfaceKind surfaceKind;
  final String routeSegment;
  final String? localeNamespace;
  final List<String> permissions;
  final MobileNavMeta nav;
  final List<MobileChildPage> childPages;
}
