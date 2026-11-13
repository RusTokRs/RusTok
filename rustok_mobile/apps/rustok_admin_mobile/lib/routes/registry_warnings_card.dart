import 'package:flutter/material.dart';

import '../registry/registry_adaptation_summary.dart';

class RegistryWarningsCard extends StatelessWidget {
  const RegistryWarningsCard({
    super.key,
    required this.summary,
  });

  final RegistryAdaptationSummary summary;

  @override
  Widget build(BuildContext context) {
    if (!summary.hasWarnings) {
      return const SizedBox.shrink();
    }

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      color: Theme.of(context).colorScheme.errorContainer,
      child: ListTile(
        title: const Text('Registry adaptation warnings'),
        subtitle: Text(summary.message),
      ),
    );
  }
}
