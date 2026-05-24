import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_admin_mobile/registry/mobile_module_registry.dart';

void main() {
  test('buildAdaptedMobileModuleRegistry returns non-empty immutable list', () {
    final routes = buildAdaptedMobileModuleRegistry();

    expect(routes, isNotEmpty);
    expect(() => routes.add(routes.first), throwsUnsupportedError);
    expect(routes.first.path, startsWith('/modules/'));
  });
}
