import 'mobile_nav_meta.dart';

class MobileModuleEntry {
  const MobileModuleEntry({
    required this.moduleKey,
    required this.routeSegment,
    required this.nav,
  });

  final String moduleKey;
  final String routeSegment;
  final MobileNavMeta nav;
}
