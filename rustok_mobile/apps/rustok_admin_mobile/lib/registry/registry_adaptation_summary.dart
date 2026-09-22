import 'module_entry_adapter.dart';

class RegistryAdaptationSummary {
  const RegistryAdaptationSummary({
    required this.hasWarnings,
    required this.message,
  });

  final bool hasWarnings;
  final String message;
}

RegistryAdaptationSummary buildRegistryAdaptationSummary(
  ModuleRegistryAdaptationResult report,
) {
  final hasWarnings =
      report.rejectedModuleEntries > 0 || report.rejectedChildEntries > 0;
  if (!hasWarnings) {
    return const RegistryAdaptationSummary(
      hasWarnings: false,
      message: 'Registry adaptation completed with no rejected entries.',
    );
  }

  return RegistryAdaptationSummary(
    hasWarnings: true,
    message:
        'Rejected modules: ${report.rejectedModuleEntries} · Rejected child pages: ${report.rejectedChildEntries}',
  );
}
