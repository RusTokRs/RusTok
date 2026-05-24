import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_admin_mobile/registry/mobile_module_registry.dart';
import 'package:rustok_admin_mobile/registry/mobile_manifest.g.dart';

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

  test('registry is sourced from generated manifest without manual host list', () {
    final entries = buildMobileModuleRegistry();

    expect(entries, hasLength(generatedMobileManifest.length));
    expect(entries.map((entry) => entry.moduleKey),
        generatedMobileManifest.map((entry) => entry.moduleKey));
  });
}
