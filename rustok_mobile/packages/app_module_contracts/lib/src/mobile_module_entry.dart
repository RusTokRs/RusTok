import 'mobile_child_page.dart';
import 'mobile_nav_meta.dart';

class MobileModuleEntry {
  const MobileModuleEntry({
    required this.moduleKey,
    required this.routeSegment,
    required this.nav,
    this.childPages = const <MobileChildPage>[],
  });

  final String moduleKey;
  final String routeSegment;
  final MobileNavMeta nav;
  final List<MobileChildPage> childPages;
}
