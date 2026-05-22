import 'package:app_module_contracts/app_module_contracts.dart';

import 'mobile_manifest.g.dart';

List<MobileModuleEntry> buildMobileModuleRegistry() {
  return List.unmodifiable(generatedMobileManifest);
}
