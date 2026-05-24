import 'package:app_module_contracts/app_module_contracts.dart';

import 'mobile_manifest.g.dart';
import 'module_entry_adapter.dart';

List<MobileModuleEntry> buildMobileModuleRegistry() {
  return List.unmodifiable(generatedMobileManifest);
}

List<ModuleRouteEntry> buildAdaptedMobileModuleRegistry() {
  return buildAdaptedMobileModuleRegistryWithReport().routes;
}

ModuleRegistryAdaptationResult buildAdaptedMobileModuleRegistryWithReport() {
  return adaptModuleEntriesWithReport(buildMobileModuleRegistry());
}
