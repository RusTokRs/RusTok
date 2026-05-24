import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_admin_mobile/registry/mobile_module_registry.dart';

void main() {
  test('buildAdaptedMobileModuleRegistry returns non-empty immutable list', () {
    final routes = buildAdaptedMobileModuleRegistry();

    expect(routes, isNotEmpty);
    expect(() => routes.add(routes.first), throwsUnsupportedError);
    expect(routes.first.path, startsWith('/modules/'));
  });

  test('buildAdaptedMobileModuleRegistryWithReport returns stable counters', () {
    final report = buildAdaptedMobileModuleRegistryWithReport();

    expect(report.routes, isNotEmpty);
    expect(report.rejectedModuleEntries, greaterThanOrEqualTo(0));
    expect(report.rejectedChildEntries, greaterThanOrEqualTo(0));
  });
}
