import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_admin_mobile/registry/module_entry_adapter.dart';
import 'package:rustok_admin_mobile/registry/registry_adaptation_summary.dart';

void main() {
  test('builds clean summary when report has no rejected entries', () {
    const report = ModuleRegistryAdaptationResult(
      routes: <ModuleRouteEntry>[],
      rejectedModuleEntries: 0,
      rejectedChildEntries: 0,
    );

    final summary = buildRegistryAdaptationSummary(report);

    expect(summary.hasWarnings, isFalse);
    expect(
      summary.message,
      'Registry adaptation completed with no rejected entries.',
    );
  });

  test('builds warning summary when report has rejected entries', () {
    const report = ModuleRegistryAdaptationResult(
      routes: <ModuleRouteEntry>[],
      rejectedModuleEntries: 3,
      rejectedChildEntries: 5,
    );

    final summary = buildRegistryAdaptationSummary(report);

    expect(summary.hasWarnings, isTrue);
    expect(summary.message, 'Rejected modules: 3 · Rejected child pages: 5');
  });
}
